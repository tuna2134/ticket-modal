use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use dotenvy::dotenv;
use sqlx::mysql::MySqlPool;
use std::env;
use std::sync::Arc;
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::{Event, Intents, Shard, ShardId};
use twilight_http::client::InteractionClient;
use twilight_http::request::application;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::{InteractionData, InteractionType};
use twilight_model::channel::message::component::{
    ActionRow, Button, ButtonStyle, Component, TextInput, TextInputStyle,
};
use twilight_model::http::interaction::{InteractionResponse, InteractionResponseType};
use twilight_model::id::Id;
use twilight_model::oauth::Application;
use twilight_model::user::CurrentUser;
use twilight_util::builder::{embed::EmbedBuilder, InteractionResponseDataBuilder};

use serde::{Deserialize, Serialize};

#[get("/")]
async fn index<'a>(state: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok().body(format!("My name is {}", state.user.name))
}

#[derive(Clone)]
struct AppState {
    http: Arc<HttpClient>,
    cache: Arc<InMemoryCache>,
    user: CurrentUser,
    pool: MySqlPool,
    application: Application,
}

#[derive(Serialize, Deserialize)]
struct TicketTemplateData {
    name: String,
    title: String,
    placeholder: String,
}

#[derive(Serialize, Deserialize)]
struct TicketData {
    title: String,
    description: String,
    template: String,
    data: Vec<TicketTemplateData>,
}

#[post("/tickets/{channel_id}")]
async fn create_ticket(
    state: web::Data<AppState>,
    info: web::Path<(u64,)>,
    data: web::Json<TicketData>,
) -> impl Responder {
    let response = state
        .http
        .create_message(Id::new(info.into_inner().0))
        .embeds(&[EmbedBuilder::new()
            .title(&data.title)
            .description(&data.description)
            .build()])
        .unwrap()
        .components(&[Component::ActionRow(ActionRow {
            components: vec![Component::Button(Button {
                custom_id: Some("create_ticket".to_owned()),
                disabled: false,
                emoji: None,
                label: Some("チケットの作成".to_owned()),
                style: ButtonStyle::Primary,
                url: None,
            })],
        })])
        .unwrap()
        .await
        .unwrap();
    let message = response.model().await.unwrap();
    sqlx::query!(
        "INSERT INTO ticket VALUES (?, ?, ?, ?)",
        message.id.get(),
        data.title,
        data.description,
        data.template
    )
    .execute(&state.pool)
    .await
    .unwrap();
    for tpl_data in data.data.iter() {
        sqlx::query!(
            "INSERT INTO ticket_template VALUES (?, ?, ?, ?)",
            message.id.get(),
            tpl_data.name,
            tpl_data.title,
            tpl_data.placeholder
        )
        .execute(&state.pool)
        .await
        .unwrap();
    }
    HttpResponse::Ok().body(format!("Created message with ID {}", message.id))
}

async fn catch_event(event: Event, state: Arc<AppState>) -> anyhow::Result<()> {
    match event {
        Event::Ready(_) => {
            println!("{} is ready!", state.user.name);
        }
        Event::InteractionCreate(interaction) => {
            let interaction_http = state.http.interaction(state.application.id);
            match interaction.kind {
                InteractionType::MessageComponent => {
                    if let InteractionData::MessageComponent(data) =
                        interaction.data.clone().unwrap()
                    {
                        println!("c: {:?}", data.custom_id);
                        if data.custom_id == "create_ticket" {
                            let res = interaction_http
                                .create_response(
                                    interaction.id,
                                    &interaction.token,
                                    &InteractionResponse {
                                        kind: InteractionResponseType::Modal,
                                        data: Some(
                                            InteractionResponseDataBuilder::new()
                                                .title("チケットの作成")
                                                .components([Component::TextInput(TextInput {
                                                    custom_id: "ticket_title".to_string(),
                                                    label: "title".to_string(),
                                                    max_length: None,
                                                    min_length: None,
                                                    placeholder: None,
                                                    required: Some(true),
                                                    style: TextInputStyle::Short,
                                                    value: None,
                                                })])
                                                .build(),
                                        ),
                                    },
                                )
                                .await?;
                            println!("{:?}", res);
                        }
                    }
                }
                _ => {}
            };
        }
        _ => {}
    }
    Ok(())
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    dotenv().ok();
    let http = Arc::new(HttpClient::new(env::var("DISCORD_TOKEN")?));
    let intents = Intents::GUILDS | Intents::GUILD_MEMBERS;
    let mut shard = Shard::new(ShardId::ONE, env::var("DISCORD_TOKEN")?, intents);
    let cache = Arc::new(InMemoryCache::new());
    let pool = MySqlPool::connect(&env::var("DATABASE_URL")?).await?;
    let user = http.current_user().await?.model().await?;
    let application = http.current_user_application().await?.model().await?;
    sqlx::migrate!().run(&pool).await?;
    let state = AppState {
        http: Arc::clone(&http),
        cache: Arc::clone(&cache),
        user,
        pool,
        application,
    };
    let bot_state = Arc::new(state.clone());
    tokio::spawn(async move {
        loop {
            let event = match shard.next_event().await {
                Ok(event) => event,
                Err(source) => {
                    if source.is_fatal() {
                        break;
                    }

                    continue;
                }
            };

            cache.update(&event);
            tokio::spawn(catch_event(event, Arc::clone(&bot_state)));
        }
    });
    HttpServer::new(move || {
        App::new()
            .service(index)
            .service(create_ticket)
            .app_data(web::Data::new(state.clone()))
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await?;
    Ok(())
}
