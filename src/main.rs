use postgres::{NoTls, Error as PostgresError};
use r2d2::{Pool};
use r2d2_postgres::PostgresConnectionManager;

use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::Arc;

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

// Gunakan env variable dari container
const DB_URL: &str = env!("DATABASE_URL");

// HTTP RESPONSES DENGAN CORS
const OK_RESPONSE: &str = 
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n";

const NOT_FOUND: &str = 
    "HTTP/1.1 404 NOT FOUND\r\nAccess-Control-Allow-Origin: *\r\n\r\n";

const INTERNAL_ERROR: &str = 
    "HTTP/1.1 500 INTERNAL ERROR\r\nAccess-Control-Allow-Origin: *\r\n\r\n";

const UNAUTHORIZED: &str = 
    "HTTP/1.1 401 UNAUTHORIZED\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n";

type PgPool = Pool<PostgresConnectionManager<NoTls>>;

fn main() {
    let manager = PostgresConnectionManager::new(DB_URL.parse().unwrap(), NoTls);
    let pool = Arc::new(PgPool::new(manager).unwrap());

    // Init database
    set_database(&pool).expect("Failed to init db");

    // Start server
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    println!("Server running on port 8080");

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let pool = Arc::clone(&pool);
            std::thread::spawn(move || {
                handle_client(stream, pool);
            });
        }
    }
}

fn handle_client(mut stream: TcpStream, pool: Arc<PgPool>) {
    let mut buffer = [0; 2048];
    let mut request = String::new();

    if let Ok(size) = stream.read(&mut buffer) {
        request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

        let (status, content) = if request.starts_with("OPTIONS") {
            handle_options()
        } else {
            match &*request {
                r if r.starts_with("POST /login")    => handle_login(r, &pool),
                r if r.starts_with("POST /users")    => handle_create(r, &pool),
                r if r.starts_with("GET /users/")    => handle_get_by_id(r, &pool),
                r if r.starts_with("GET /users")     => handle_get_all(&pool),
                r if r.starts_with("PUT /users/")    => handle_update(r, &pool),
                r if r.starts_with("DELETE /users/") => handle_delete(r, &pool),
                _ => (NOT_FOUND.to_string(), "404 not found".to_string()),
            }
        };

        let _ = stream.write_all(format!("{}{}", status, content).as_bytes());
    }
}

// ================= HANDLERS =================

fn handle_options() -> (String, String) {
    (
        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET,POST,PUT,DELETE,OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n".into(),
        "".into()
    )
}

fn handle_create(req: &str, pool: &PgPool) -> (String, String) {
    let body = match get_user_body(req) {
        Ok(b) => b,
        Err(_) => return (INTERNAL_ERROR.into(), "Invalid body".into()),
    };

    let mut client = pool.get().unwrap();
    let _ = client.execute(
        "INSERT INTO users (name, email, password) VALUES ($1,$2,$3)",
        &[&body.name, &body.email, &body.password],
    );

    (OK_RESPONSE.into(), "{\"message\":\"created\"}".into())
}

fn handle_login(req: &str, pool: &PgPool) -> (String, String) {
    let body = match get_login_body(req) {
        Ok(b) => b,
        Err(_) => return (INTERNAL_ERROR.into(), "Invalid body".into()),
    };

    let mut client = pool.get().unwrap();
    let rows = client.query(
        "SELECT id,name FROM users WHERE name=$1 AND password=$2",
        &[&body.name, &body.password],
    );

    match rows {
        Ok(r) if !r.is_empty() => {
            let name: String = r[0].get(1);
            (OK_RESPONSE.into(), format!("{{\"success\":true,\"name\":\"{}\"}}", name))
        }
        _ => (UNAUTHORIZED.into(), "{\"success\":false,\"message\":\"Invalid login\"}".into()),
    }
}

fn handle_get_by_id(req: &str, pool: &PgPool) -> (String, String) {
    let id: i32 = match get_id(req).parse() {
        Ok(id) => id,
        Err(_) => return (INTERNAL_ERROR.into(), "Invalid ID".into()),
    };

    let mut client = pool.get().unwrap();
    let res = client.query_opt("SELECT * FROM users WHERE id=$1", &[&id]);

    match res {
        Ok(Some(row)) => {
            let user = User { id: row.get(0), name: row.get(1), email: row.get(2), password: row.get(3) };
            (OK_RESPONSE.into(), serde_json::to_string(&user).unwrap())
        }
        _ => (NOT_FOUND.into(), "Not found".into()),
    }
}

fn handle_get_all(pool: &PgPool) -> (String, String) {
    let mut client = pool.get().unwrap();
    let rows = client.query("SELECT * FROM users", &[]).unwrap();
    let users: Vec<User> = rows.iter().map(|r| User { id: r.get(0), name: r.get(1), email: r.get(2), password: r.get(3) }).collect();
    (OK_RESPONSE.into(), serde_json::to_string(&users).unwrap())
}

fn handle_update(req: &str, pool: &PgPool) -> (String, String) {
    let id: i32 = match get_id(req).parse() { Ok(id) => id, Err(_) => return (INTERNAL_ERROR.into(), "Invalid ID".into()) };
    let body = match get_user_body(req) { Ok(b) => b, Err(_) => return (INTERNAL_ERROR.into(), "Invalid body".into()) };

    let mut client = pool.get().unwrap();
    client.execute("UPDATE users SET name=$1,email=$2,password=$3 WHERE id=$4", &[&body.name, &body.email, &body.password, &id]).unwrap();
    (OK_RESPONSE.into(), "{\"message\":\"updated\"}".into())
}

fn handle_delete(req: &str, pool: &PgPool) -> (String, String) {
    let id: i32 = match get_id(req).parse() { Ok(id) => id, Err(_) => return (INTERNAL_ERROR.into(), "Invalid ID".into()) };
    let mut client = pool.get().unwrap();
    let rows = client.execute("DELETE FROM users WHERE id=$1", &[&id]).unwrap();

    if rows == 0 { return (NOT_FOUND.into(), "Not found".into()); }
    (OK_RESPONSE.into(), "{\"message\":\"deleted\"}".into())
}

// ================= HELPERS =================

fn set_database(pool: &PgPool) -> Result<(), PostgresError> {
    let mut client = pool.get().unwrap();
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

fn get_id(req: &str) -> &str { req.split("/").nth(2).unwrap_or("").split_whitespace().next().unwrap_or("") }
fn get_user_body(req: &str) -> Result<User, serde_json::Error> { serde_json::from_str(req.split("\r\n\r\n").last().unwrap_or("")) }
fn get_login_body(req: &str) -> Result<LoginBody, serde_json::Error> { serde_json::from_str(req.split("\r\n\r\n").last().unwrap_or("")) }
