use postgres::{Client, NoTls};
use postgres::Error as PostgresError;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::env;

#[macro_use]
extern crate serde_derive;

#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct LoginBody {
    name: String,
    password: String,
}

const DB_URL: &str = env!("DATABASE_URL");

const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_ERROR: &str = "HTTP/1.1 500 INTERNAL ERROR\r\n\r\n";
const UNAUTHORIZED: &str = "HTTP/1.1 401 UNAUTHORIZED\r\nContent-Type: application/json\r\n\r\n";

fn main() {
    if let Err(_) = set_database() {
        return;
    }

    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    println!("Server listening on port 8080");

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            handle_client(stream);
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let mut request = String::new();

    if let Ok(size) = stream.read(&mut buffer) {
        request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

        let (status_line, content) = match &*request {
            r if r.starts_with("POST /login") => handle_login_request(r),   // login user
            r if r.starts_with("POST /users") => handle_post_request(r),    // create user
            r if r.starts_with("GET /users/") => handle_get_request(r),     // get user by id
            r if r.starts_with("GET /users") => handle_get_all_request(r),  // get all users
            r if r.starts_with("PUT /users/") => handle_put_request(r),     // update user
            r if r.starts_with("DELETE /users/") => handle_delete_request(r), // delete user
            _ => (NOT_FOUND.to_string(), "404 not found".to_string()),
        };

        let _ = stream.write_all(format!("{}{}", status_line, content).as_bytes());
    }
}

//
// POST /login — cek username & password di database
//
fn handle_login_request(request: &str) -> (String, String) {
    match (get_login_body(request), Client::connect(DB_URL, NoTls)) {
        (Ok(body), Ok(mut client)) => {
            let rows = client.query(
                "SELECT id, name FROM users WHERE name = $1 AND password = $2",
                &[&body.name, &body.password],
            );

            match rows {
                Ok(r) if !r.is_empty() => {
                    let name: String = r[0].get(1);
                    let res = format!("{{\"success\":true,\"message\":\"Login berhasil\",\"name\":\"{}\"}}", name);
                    (OK_RESPONSE.to_string(), res)
                }
                _ => (
                    UNAUTHORIZED.to_string(),
                    "{\"success\":false,\"message\":\"Username atau password salah\"}".to_string(),
                ),
            }
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//
// POST /users — tambah user baru
//
fn handle_post_request(request: &str) -> (String, String) {
    match (get_user_request_body(request), Client::connect(DB_URL, NoTls)) {
        (Ok(user), Ok(mut client)) => {
            client.execute(
                "INSERT INTO users (name, email, password) VALUES ($1, $2, $3)",
                &[&user.name, &user.email, &user.password],
            ).unwrap();

            (OK_RESPONSE.to_string(), "User created".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//
// GET /users/{id} — ambil user berdasarkan ID
//
fn handle_get_request(request: &str) -> (String, String) {
    match (get_id(request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => match client.query_one("SELECT * FROM users WHERE id = $1", &[&id]) {
            Ok(row) => {
                let user = User {
                    id: row.get(0),
                    name: row.get(1),
                    email: row.get(2),
                    password: row.get(3),
                };

                (OK_RESPONSE.to_string(), serde_json::to_string(&user).unwrap())
            }
            _ => (NOT_FOUND.to_string(), "User not found".to_string()),
        },
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//
// GET /users — ambil semua user
//
fn handle_get_all_request(_: &str) -> (String, String) {
    match Client::connect(DB_URL, NoTls) {
        Ok(mut client) => {
            let mut users = Vec::new();

            for row in client.query("SELECT id, name, email, password FROM users", &[]).unwrap() {
                users.push(User {
                    id: row.get(0),
                    name: row.get(1),
                    email: row.get(2),
                    password: row.get(3),
                });
            }

            (OK_RESPONSE.to_string(), serde_json::to_string(&users).unwrap())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//
// PUT /users/{id} — update user
//
fn handle_put_request(request: &str) -> (String, String) {
    match (
        get_id(request).parse::<i32>(),
        get_user_request_body(request),
        Client::connect(DB_URL, NoTls),
    ) {
        (Ok(id), Ok(user), Ok(mut client)) => {
            client.execute(
                "UPDATE users SET name = $1, email = $2, password = $3 WHERE id = $4",
                &[&user.name, &user.email, &user.password, &id],
            ).unwrap();

            (OK_RESPONSE.to_string(), "User updated".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//
// DELETE /users/{id} — hapus user
//
fn handle_delete_request(request: &str) -> (String, String) {
    match (get_id(request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            let affected = client.execute("DELETE FROM users WHERE id = $1", &[&id]).unwrap();

            if affected == 0 {
                return (NOT_FOUND.to_string(), "User not found".to_string());
            }

            (OK_RESPONSE.to_string(), "User deleted".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

fn set_database() -> Result<(), PostgresError> {
    let mut client = Client::connect(DB_URL, NoTls)?;
    client.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL,
            password VARCHAR NOT NULL
        )
    "
    )?;
    Ok(())
}

fn get_id(request: &str) -> &str {
    request.split("/").nth(2).unwrap_or_default().split_whitespace().next().unwrap_or_default()
}

fn get_user_request_body(request: &str) -> Result<User, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

fn get_login_body(request: &str) -> Result<LoginBody, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}
