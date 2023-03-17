use actix_web::http::header::CONTENT_LENGTH;
use crate::domain::SubscriberEmail;
use reqwest::Client;
use reqwest::Url;
use secrecy::{ExposeSecret, Secret};

pub struct EmailClient {
    sender: SubscriberEmail,
    //base_url: Url,
    base_url: String,
    http_client: Client,
    // we dont want to log this by accident
    authorization_token: Secret<String>
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail, authorization_token: Secret<String>, timeout: std::time::Duration,) -> Self{
        let http_client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap();
        Self {
            http_client,
            //base_url: Url::parse(&base_url).unwrap(),
            base_url,
            sender,
            authorization_token
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str
    ) -> Result<(), reqwest::Error> {
        // You can do better using 'reqwest::Url::join' if you change
        // 'base_url' 's type from 'String' to reqwest::Url'.
        let url = format!("{}/email", self.base_url);
        //let url=  self.base_url.join("/email").unwrap();

        //allocates a bunch of new memory to store a cloned String
        //lets try to reference the existing data
        // make the definition for sendemailrequest use a string slice
        // a string slice is just a pointer to a memory buffer owned by someone else
        //to store a reference in a struct we need to add a lifetime parameter
        // which keeps track of how long the references are valid for
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject: subject,
            html_body: html_content,
            text_body: text_content,
        };
        self.http_client
            .post(&url)
            .header("X-Postmark-Server-Token",
                self.authorization_token.expose_secret())
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

#[cfg(test)]
mod tests {
    use std::borrow::Borrow;
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph,Sentence};
    use fake::{Fake,Faker};
    use wiremock::matchers::{header_exists, header, path, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use secrecy::Secret;
    use wiremock::Request;
    use wiremock::matchers::any;
    use claim::assert_ok;
    use claim::assert_err;

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            // try to parse the body as a json value
            let result: Result<serde_json::Value, _> =
                serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                //check that all the mandatory fields are populated
                // without inspecting the field values
                dbg!(&body);
                body.get("From").is_some()
                && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else{
                false
            }
        }
    }

    // Generate a random email subject
    fn subject() -> String {
        Sentence(1..2).fake()
    }

    // Generate a random email content
    fn content() -> String {
        Paragraph(1..10).fake()
    }

    // Generate a random subscriber email
    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    // get a test instance of 'EmailClient'
    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(base_url, email(), Secret::new(Faker.fake()), std::time::Duration::from_millis(200))
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(header_exists("X-Postmark-Server-Token"))
                            .and(header("Content-Type", "application/json"))
                            .and(path("/email"))
                            .and(method("POST"))
                            .and(SendEmailBodyMatcher)
                            .respond_with(ResponseTemplate::new(200))
                            .expect(1)
                            .mount(&mock_server)
                            .await;

        // Act
        let _ = email_client
            .send_email(email().borrow(), &subject(), &content(), &content())
            .await;

        //Assert
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        //we do not copy in all the matchers we have in the other test.
        // the purpose of this test is not to assert on the request we are sending out
        // we add the bare minimum needed to trigger the path we want to test in 'send_email'
        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send_email(email().borrow(), &subject(), &content(), &content())
            .await;
        //assert
        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        //we do not copy in all the matchers we have in the other test.
        // the purpose of this test is not to assert on the request we are sending out
        // we add the bare minimum needed to trigger the path we want to test in 'send_email'
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send_email(email().borrow(), &subject(), &content(), &content())
            .await;
        //assert
        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        //Arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        let response = ResponseTemplate::new(200)
            .set_delay(std::time::Duration::from_secs(180));

        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send_email(email().borrow(), &subject(), &content(), &content())
            .await;

        // Assert
        assert_err!(outcome);
    }

}