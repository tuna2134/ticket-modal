-- Add migration script here
CREATE TABLE IF NOT EXISTS ticket (
    id BIGINT PRIMARY KEY,
    title TEXT,
    description TEXT,
    template TEXT
);

CREATE TABLE IF NOT EXISTS ticket_template (
    ticket_id BIGINT,
    name TEXT,
    title TEXT,
    placeholder TEXT,
    FOREIGN KEY (ticket_id) REFERENCES ticket(id)
);