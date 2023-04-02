use std::net::TcpListener;
use actix_web::App;
use once_cell::sync::Lazy;
use sqlx::{Connection,Executor, PgConnection, PgPool};
use zero2Prod::configuration::{get_configuration, DatabaseSettings};
use sqlx::types::Uuid;
use zero2Prod::email_client::EmailClient;
use zero2Prod::telemetry::{get_subscriber, init_subscriber};
use zero2Prod::startup::{get_connection_pool, Application};
use wiremock::MockServer;
//use sha3::Digest;
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};

// Ensure that the 'tracing' stack is only initialised once using 'once_cell'
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    // We cannot assing the output of 'get_subscriber' to a variable based on the
    // valuye TEST_LOG' because the sink is part of the type returned by get subscriber
    // therefore they are not the same type
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name,
                                        default_filter_level,
                                        std::io::stdout
        );
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name,
                                        default_filter_level,
                                        std::io::sink);
        init_subscriber(subscriber);
    };
});

//confirmation links embedded in the request to the email api
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url
}

// Generalise spawn_App
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    //new field!
    pub email_server: MockServer,
    //new field for test only
    pub port: u16,
    pub test_user: TestUser,
    //define single client instance for every helper method, allows us to store cookies
    pub api_client: reqwest::Client,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
    // extract the confirmation links embedded in the request to the email api.
    // add to test app to access application port to inject links
    pub fn get_confirmation_links(
        &self,
        email_request: &wiremock::Request
    ) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(
            &email_request.body
        ).unwrap();

        // Extract the link from one of the request fields.
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);

            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // make sure we dont call random apis on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };
        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());
        ConfirmationLinks {
            html,
            plain_text
        }
    }

    pub async fn get_publish_newsletter(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_publish_newsletter_html(&self) -> String {
        self.get_publish_newsletter().await.text().await.unwrap()
    }

    pub async fn post_publish_newsletter<Body>(&self, body: &Body) -> reqwest::Response
        where
            Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/newsletters", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }


    //retrieve username and password
    pub async fn test_user(&self) -> (String, String) {
        let row = sqlx::query!("SELECT username, password_hash FROM users LIMIT 1",)
            .fetch_one(&self.db_pool)
            .await
            .expect("Failed to create test users.");
        (row.username, row.password_hash)
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
        {
            self.api_client
                .post(&format!("{}/login", &self.address))
                // This 'reqwest' method makes sure that the body is URL-encoded
                // and the 'Content-type' header is set accordingly.
                .form(body)
                .send()
                .await
                .expect("Failed to execute request.")
        }

    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn get_change_password(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_change_password<Body>(&self, body:&Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

// Launch our application in the background ~somehow~

// No .await call, therefore no need for `spawn_app` to be async now.
// We are also running tests, so it is not worth it to propagate errors:
// if we fail to perform the required setup we can just panic and crash
// all the things.

//public now!
pub async fn spawn_app() -> TestApp {
    // the first time 'initialize' is invoked the code in 'TRAICNG' is executed.
    // All other invocations will instead skip execution.
    Lazy::force(&TRACING);
    /*
    //zero2Prod::run().await
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind random port");
    // We retrieve the port assigned to us by the OS
    let port = listener.local_addr().unwrap().port();

    let address = format!("http://127.0.0.1:{}", port);
    */

    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        //use mock server as email api
        c.email_client.base_url = email_server.uri();
        c
    };

    configure_database(&configuration.database).await;

    //let connection_pool = configure_database(&configuration.database).await;
    /*
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");
    */
/*
    //Build new email client
    let sender_email = configuration.email_client.sender().expect("Invalid sender email address.");
    let timeout = configuration.email_client.timeout();
    let email_client = EmailClient::new(configuration.email_client.base_url, sender_email, configuration.email_client.authorization_token, timeout);

    let server = zero2Prod::startup::run(listener, connection_pool.clone(), email_client)
        .expect("Failed to bind address");

 */
    //let server = build(configuration).await.expect("Failed to build application.");

    let application = Application::build(configuration.clone()).await
        .expect("Failed to build application.");
    let application_port = application.port();

    let client= reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();
    // Launch the server as a background task
    // tokio::spawn returns a handle to the spawned future,
    // but we have no use for it here, hence the non-binding let
    let _ = tokio::spawn(application.run_until_stopped());
    let test_app = TestApp {
        address: format!("http://localhost:{}", application_port),
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        port: application_port,
        test_user: TestUser::generate(),
        api_client: client,
    };
    test_app.test_user.store(&test_app.db_pool).await;
    test_app
    // We return the application address to the caller!
    //format!("http://127.0.0.1:{}", port)
}

//not public anymore
async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // Create Database
    let mut connection = PgConnection::connect_with(
        &config.without_db())
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // Migrate database
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");
    connection_pool
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string()
            //password: "everythinghastostartsomewhere".into(),
        }
    }

    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        // Match parameters of the default password
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        /*
        // we dont care about the exact Argon2 parameters here
        // given that it's for testing purposes!
        let password_hash = Argon2::default()
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        */
       /*
        let password_hash = sha3::Sha3_256::digest(
            self.password.as_bytes()
        );
        let password_hash = format!("{:x}", password_hash);
        */
        //dbg!(&password_hash);
        sqlx::query!(
            "INSERT INTO users (user_id, username, password_hash)\
            VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash,
        )
            .execute(pool)
            .await
            .expect("Failed to store test user.");
    }

    pub async fn login(&self, app: &TestApp) {
        app.post_login(&serde_json::json!({
            "username": &self.username,
            "password": &self.password
        }))
            .await;
    }

}

//little helper function - we will be doing this check several times.
pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
