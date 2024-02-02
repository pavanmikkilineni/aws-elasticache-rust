extern crate redis;
use redis::Commands;
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, prelude::FromRow, query_as, sqlite::SqlitePoolOptions, Sqlite};
use dotenv::dotenv;

// Entry point of the application
#[tokio::main]
async fn main() {
    dotenv().ok(); 

    const DB_URL: &str = "sqlite://user.db";

    // Check if the database exists, if not, create it
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        println!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    } else {
        println!("Database already exists");
    }

    // Connect to the database
    let db = match SqlitePoolOptions::new()
        .max_connections(10)
        .connect(DB_URL)
        .await
    {
        Ok(db) => {
            println!("✅ Connection to the database is successful!");
            db
        }
        Err(err) => {
            println!("🔥 Failed to connect to the database: {:?}", err);
            std::process::exit(1);
        }
    };

    let result = sqlx::query("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY NOT NULL, name VARCHAR(250) NOT NULL);").execute(&db).await.unwrap();
    println!("Create user table result: {:?}", result);

    let user_result = query_as::<_, User>(
        "INSERT INTO users (id, name) VALUES (?, ?) RETURNING id, name",
    )
    .bind(1)
    .bind("Pavan".to_string())
    .fetch_one(&db)
    .await;

    match user_result{
        Ok(user) => {
            println!("✅ Created user: {:?}",user);
        }
        Err(err) => {
            println!("🔥 Failed to user: {:?}", err);
            std::process::exit(1);
        }
    }

    // Open a connection
    let redis_connection_url = std::env::var("REDIS_CONNECTION_URL").expect("REDIS_CONNECTION_URL must be set.");
    let mut client = redis::Client::open(redis_connection_url).unwrap();

    //Check the Cache
    match client.get::<i32, String>(1) {      
        Ok(serialized_user) => {
            let user: User = serde_json::from_str(&serialized_user).unwrap();
            println!("User from cache: {:?}", user)
        }
        Err(_) => {
            // Retrieve the user from Sqlite if the user is not in the cache
            let user_result = sqlx::query_as::<_, User>("SELECT id, name FROM users where id = ?")
                .bind(1)
                .fetch_one(&db)
                .await;

            match user_result {
                Ok(user) => {
                    let serialized_user = serde_json::to_string(&user).unwrap();
                    //Populate the cache
                    let _: () = client.set(1, serialized_user).unwrap();
                }
                Err(_) => {
                    // Handle the case when the User with the specified ID is not found
                    println!("User with ID: {} not found", 1);
                }
            }
        }
    }
}

#[derive(Clone, FromRow, Debug, Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
}
