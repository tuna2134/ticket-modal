import requests


r = requests.post("http://localhost:8000/tickets/972501191214907482", json={
    "title": "Test",
    "description": "Test",
    "template": "test: ${test}",
    "data": [
        {
            "name": "test",
            "title": "Test",
            "placeholder": "test text",
        }
    ]
})

print(r.content)
