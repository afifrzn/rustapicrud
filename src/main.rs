use postgres::{ Client, NoTls };
use postgres::Error as PostgresError;
use std::net::{ TcpListener, TcpStream };
use std::io::{ Read, Write };
use std::env;

#[macro_use]
extern crate serde_derive;

//Model: User struct with id, name, email
#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
    password: String,
}

//Model: Mapel struct
#[derive(Serialize, Deserialize)]
struct Mapel {
    id: Option<i32>,
    mapel: String,
}

#[derive(Serialize, Deserialize)]
struct Guru {
    id: Option<i32>,
    name: String,
    nomor_telefon: String,
}

//DATABASE URL
const DB_URL: &str = env!("DATABASE_URL");

//constants
const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_ERROR: &str = "HTTP/1.1 500 INTERNAL ERROR\r\n\r\n";

//main function
fn main() {
    //Set Database with retry logic
    let max_retries = 10;
    let mut retry_count = 0;
    
    loop {
        match set_database() {
            Ok(_) => {
                println!("Database connected and tables created successfully");
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    println!("Error setting database after {} retries: {}", max_retries, e);
                    return;
                }
                println!("Database connection attempt {} failed: {}. Retrying in 2 seconds...", retry_count, e);
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }

    //start server and print port
    let listener = TcpListener::bind(format!("0.0.0.0:8080")).unwrap();
    println!("Server listening on port 8080");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(e) => {
                println!("Unable to connect: {}", e);
            }
        }
    }
}

//handle requests
fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let mut request = String::new();

    match stream.read(&mut buffer) {
        Ok(size) => {
            request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

            let (status_line, content) = match &*request {
                r if r.starts_with("POST /users") => handle_post_user(r),
                r if r.starts_with("GET /users/") => handle_get_user(r),
                r if r.starts_with("GET /users") => handle_get_all_users(r),
                r if r.starts_with("PUT /users/") => handle_put_user(r),
                r if r.starts_with("DELETE /users/") => handle_delete_user(r),
                r if r.starts_with("POST /mapel") => handle_post_mapel(r),
                r if r.starts_with("GET /mapel/") => handle_get_mapel(r),
                r if r.starts_with("GET /mapel") => handle_get_all_mapel(r),
                r if r.starts_with("PUT /mapel/") => handle_put_mapel(r),
                r if r.starts_with("DELETE /mapel/") => handle_delete_mapel(r),
                r if r.starts_with("POST /guru") => handle_post_mapel(r),
                r if r.starts_with("GET /guru/") => handle_get_mapel(r),
                r if r.starts_with("GET /guru") => handle_get_all_mapel(r),
                r if r.starts_with("PUT /guru/") => handle_put_mapel(r),
                r if r.starts_with("DELETE /guru/") => handle_delete_mapel(r),
                _ => (NOT_FOUND.to_string(), "404 not found".to_string()),
            };

            stream.write_all(format!("{}{}", status_line, content).as_bytes()).unwrap();
        }
        Err(e) => eprintln!("Unable to read stream: {}", e),
    }
}

//handle post user request
fn handle_post_user(request: &str) -> (String, String) {
    match (get_user_request_body(&request), Client::connect(DB_URL, NoTls)) {
        (Ok(user), Ok(mut client)) => {
            client
                .execute(
                    "INSERT INTO users (name, email, password) VALUES ($1, $2, $3)",
                    &[&user.name, &user.email, &user.password]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User created".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle post mapel request
fn handle_post_mapel(request: &str) -> (String, String) {
    match (get_mapel_request_body(&request), Client::connect(DB_URL, NoTls)) {
        (Ok(mapel), Ok(mut client)) => {
            client
                .execute(
                    "INSERT INTO mapel (mapel) VALUES ($1)",
                    &[&mapel.mapel]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Mapel created".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

fn handle_post_guru(request: &str) -> (String, String) {
    match (get_guru_request_body(&request), Client::connect(DB_URL, NoTls)) {
        (Ok(guru), Ok(mut client)) => {
            client
                .execute(
                    "INSERT INTO guru (name, nomor_telefon) VALUES ($1, $2)",
                    &[&guru.name, &guru.nomor_telefon]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Guru created".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle get user request
fn handle_get_user(request: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) =>
            match client.query_one("SELECT * FROM users WHERE id = $1", &[&id]) {
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
            }

        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle get mapel request
fn handle_get_mapel(request: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) =>
            match client.query_one("SELECT * FROM mapel WHERE id = $1", &[&id]) {
                Ok(row) => {
                    let mapel = Mapel {
                        id: row.get(0),
                        mapel: row.get(1),
                    };

                    (OK_RESPONSE.to_string(), serde_json::to_string(&mapel).unwrap())
                }
                _ => (NOT_FOUND.to_string(), "Mapel not found".to_string()),
            }

        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

// handle get guru request
fn handle_get_guru(request: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) =>
            match client.query_one("SELECT * FROM guru WHERE id = $1", &[&id]) {
                Ok(row) => {
                    let guru = Guru {
                        id: row.get(0),
                        name: row.get(1),
                        nomor_telefon: row.get(2),
                    };

                    (OK_RESPONSE.to_string(), serde_json::to_string(&guru).unwrap())
                }
                _ => (NOT_FOUND.to_string(), "Guru not found".to_string()),
            }

        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle get all users request
fn handle_get_all_users(_request: &str) -> (String, String) {
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

//handle get all mapel request
fn handle_get_all_mapel(_request: &str) -> (String, String) {
    match Client::connect(DB_URL, NoTls) {
        Ok(mut client) => {
            let mut mapels = Vec::new();

            for row in client.query("SELECT id, mapel FROM mapel", &[]).unwrap() {
                mapels.push(Mapel {
                    id: row.get(0),
                    mapel: row.get(1),
                });
            }

            (OK_RESPONSE.to_string(), serde_json::to_string(&mapels).unwrap())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle get all gurr
fn handle_get_all_guru(_request: &str) -> (String, String) {
    match Client::connect(DB_URL, NoTls) {
        Ok(mut client) => {
            let mut guru = Vec::new();

            for row in client.query("SELECT id, name, nomor_telefon FROM guru", &[]).unwrap() {
                guru.push(Guru {
                    id: row.get(0),
                    name: row.get(1),
                    nomor_telefon: row.get(2),
                });
            }

            (OK_RESPONSE.to_string(), serde_json::to_string(&guru).unwrap())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle put user request
fn handle_put_user(request: &str) -> (String, String) {
    match
        (
            get_id(&request).parse::<i32>(),
            get_user_request_body(&request),
            Client::connect(DB_URL, NoTls),
        )
    {
        (Ok(id), Ok(user), Ok(mut client)) => {
            client
                .execute(
                    "UPDATE users SET name = $1, email = $2, password = $3 WHERE id = $4",
                    &[&user.name, &user.email, &user.password, &id]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User updated".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle put mapel request
fn handle_put_mapel(request: &str) -> (String, String) {
    match
        (
            get_id(&request).parse::<i32>(),
            get_mapel_request_body(&request),
            Client::connect(DB_URL, NoTls),
        )
    {
        (Ok(id), Ok(mapel), Ok(mut client)) => {
            client
                .execute(
                    "UPDATE mapel SET mapel = $1 WHERE id = $2",
                    &[&mapel.mapel, &id]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Mapel updated".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle put guru request
fn handle_put_user(request: &str) -> (String, String) {
    match
        (
            get_id(&request).parse::<i32>(),
            get_guru_request_body(&request),
            Client::connect(DB_URL, NoTls),
        )
    {
        (Ok(id), Ok(guru), Ok(mut client)) => {
            client
                .execute(
                    "UPDATE guru SET name = $1, nomor_telefon = $2 WHERE id = $3",
                    &[&guru.name, &user.nomor_telefon, &id]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Guru updated".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle delete user request
fn handle_delete_user(request: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client.execute("DELETE FROM users WHERE id = $1", &[&id]).unwrap();

            //if rows affected is 0, user not found
            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "User not found".to_string());
            }

            (OK_RESPONSE.to_string(), "User deleted".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle delete mapel request
fn handle_delete_mapel(request: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client.execute("DELETE FROM mapel WHERE id = $1", &[&id]).unwrap();

            //if rows affected is 0, mapel not found
            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "Mapel not found".to_string());
            }

            (OK_RESPONSE.to_string(), "Mapel deleted".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle delete guru request
fn handle_delete_guru(request: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(DB_URL, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client.execute("DELETE FROM guru WHERE id = $1", &[&id]).unwrap();

            //if rows affected is 0, user not found
            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "Guru not found".to_string());
            }

            (OK_RESPONSE.to_string(), "Guru deleted".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//db setup
fn set_database() -> Result<(), PostgresError> {
    let mut client = Client::connect(DB_URL, NoTls)?;
    client.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL,
            password VARCHAR NOT NULL
        );
        CREATE TABLE IF NOT EXISTS mapel (
            id SERIAL PRIMARY KEY,
            mapel VARCHAR NOT NULL
        );
        CREATE TABLE IF NOT EXISTS, guru (
            id SERIAL PRIMARY KEY,
            nama VARCHAR NOT NULL,
            no_telefon VARCHAR NOT NULL
    );
    "
    )?;
    Ok(())
}

//Get id from request URL
fn get_id(request: &str) -> &str {
    request.split("/").nth(2).unwrap_or_default().split_whitespace().next().unwrap_or_default()
}

//deserialize user from request body without id
fn get_user_request_body(request: &str) -> Result<User, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

//deserialize mapel from request body without id
fn get_mapel_request_body(request: &str) -> Result<Mapel, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

//deserialize guru from request body without id
fn get_guru_request_body(request: &str) -> Result<Guru, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}