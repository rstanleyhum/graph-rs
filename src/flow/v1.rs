/*
OAuth2 for Microsoft Authorization using the REST OneDrive Graph API V1:

    https://docs.microsoft.com/en-us/onedrive/developer/rest-api/?view=odsp-graph-online

Specifically the implementation is here:

    https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/msa-oauth?view=odsp-graph-online

Overview:

    client_id:
        Type:  string
        Description:  The client ID value created for your application.
    redirect_uri
        Type: string
        Description: The redirect URL that the browser is sent to when authentication is complete.
    response_type
        Type: string
        Description: The type of response expected from the authorization flow.
        Flow Type:
            1. Code: Value must be 'code'
            2. Token: Value must be 'token'
    scope
        Type: string
        Description: A space-separated list of scopes your application requires.
    refresh_token
        Type: string
        Description: The refresh token you received previously. If the flow is a
                    code flow and offline access is requested, then the response
                    from the access token (2nd step) request will have a refresh code.

TOKEN FLOW
    GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    client_id={client_id}
    &scope={scope}
    &response_type=code
    &redirect_uri={redirect_uri}

CODE FLOW
    Step 1:
        GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
        client_id={client_id}
        &scope={scope}
        &response_type=token
        &redirect_uri={redirect_uri}

    Step 2:
        POST https://login.live.com/oauth20_token.srf
        Content-Type: application/x-www-form-urlencoded

        client_id={client_id}&redirect_uri={redirect_uri}&client_secret={client_secret}
        &code={code}&grant_type=authorization_code
*/

use core::fmt;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use std::result;
use std::thread;

use reqwest::header;
use url::form_urlencoded;

use crate::drive::Drive;
use crate::flow::accesstoken::AccessToken;
use crate::flow::encode::OauthUrlBuilder;
use crate::flow::error::FlowErrorType;
use crate::process::jsonio::JsonFile;

#[derive(Debug, Copy, Clone)]
pub enum FlowType {
    AuthorizeTokenFlow,
    AuthorizeCodeFlow,
    GrantTypeAuthCode,
    GrantTypeRefreshToken,
}

impl FlowType {
    fn as_str(&self) -> &'static str {
        match *self {
            FlowType::AuthorizeTokenFlow => "token",
            FlowType::AuthorizeCodeFlow => "code",
            FlowType::GrantTypeRefreshToken => "refresh_token",
            FlowType::GrantTypeAuthCode => "authorization_code",
        }
    }
}

impl fmt::Display for FlowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowType::AuthorizeTokenFlow => write!(f, "{:#?}", "token"),
            FlowType::AuthorizeCodeFlow => write!(f, "{:#?}", "code"),
            FlowType::GrantTypeAuthCode => write!(f, "{:#?}", "authorization_code"),
            FlowType::GrantTypeRefreshToken => write!(f, "{:#?}", "refresh_token"),
        }
    }
}

pub enum FlowStatus {
    AccessCode,
    AccessToken,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AuthUrl {
    AccountAuth,
    AccountToken,
    GraphAuth,
    GraphToken,
}

impl AuthUrl {
    pub fn as_str(&self) -> &'static str {
        match *self {
            AuthUrl::AccountAuth => "https://login.live.com/oauth20_authorize.srf?",
            AuthUrl::AccountToken => "https://login.live.com/oauth20_token.srf",
            AuthUrl::GraphAuth => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?",
            AuthUrl::GraphToken => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AccountType {
    Account,
    Graph,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match *self {
            AccountType::Account => "Account",
            AccountType::Graph => "Graph",
        }
    }
}

/// Builder for the OAuth2 specification for Microsoft Graph and Authorization
///
/// # Example
///
/// Creating an AuthFlow uses the builder pattern:
/// ```
/// use rust_onedrive::flow::v1::AuthFlow;
///
///
/// let mut auth_flow = AuthFlow::new(true);
///     auth_flow.set_client_id("client_id")
///        .set_auth_url("https://example.com/authorize")
///        .set_client_secret("client_secret")
///        .set_token_url("https://example.com/token");
/// ```
///
/// There is also an enum for automatically setting the end points
/// to use based upon the type of resource: Account, Graph.
///
/// Account endpoints:
///     Access Code: https://login.live.com/oauth20_authorize.srf?
///     Access Token: "https://login.live.com/oauth20_token.srf
///
/// Graph endpoints:
///     Access Code: https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
///     Access Token: https://login.microsoftonline.com/common/oauth2/v2.0/token
///
/// # Example
/// ```
/// use rust_onedrive::flow::v1::{AuthFlow, AccountType};
///
///    let mut auth_flow = AuthFlow::new(true);
///    auth_flow
///        .set_client_id("client_id")
///        .set_redirect_uri("http://localhost:8000/redirect")
///        .set_client_secret("client_secret");
///
///    auth_flow.use_default_auth_url(AccountType::Account);
///
///    assert_eq!(auth_flow.get_auth_url().unwrap(), "https://login.live.com/oauth20_authorize.srf?");
///```
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthFlow {
    config_name: String,
    scopes: Vec<String>,
    params: HashMap<String, String>,
    allow_reset: bool,
    default_scope: bool,
    default_auth: bool,
    auth_type: AccountType,
    access_token: Option<Box<AccessToken>>,
}

impl fmt::Display for AuthFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:#?}\n{:#?}\n{:#?}\n{:#?}\n{:#?}",
            self.config_name, self.scopes, self.params, self.allow_reset, self.default_scope
        )
    }
}

impl AuthFlow {
    pub fn new(default: bool) -> AuthFlow {
        AuthFlow {
            config_name: String::from("AuthFlow"),
            scopes: Vec::new(),
            params: HashMap::new(),
            allow_reset: false,
            default_scope: default,
            default_auth: false,
            auth_type: AccountType::Account,
            access_token: None,
        }
    }

    /// Set the client id of a request
    ///Set the client id of an OAuth URL.
    ///
    /// # Example
    /// ```
    /// use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_client_id("client_id");
    /// ```
    pub fn set_client_id(&mut self, client_id: &str) -> &mut AuthFlow {
        self.set_config("CLIENT_ID", client_id)
    }

    /// Set the client secret of an OAuth URL.
    ///
    /// # Example
    /// ```
    /// use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_client_secret("client_secret");
    /// ```
    pub fn set_client_secret(&mut self, client_secret: &str) -> &mut AuthFlow {
        self.set_config("CLIENT_SECRET", client_secret)
    }

    /// Set the auth url of a request
    /// Set the authorization URL for OAuth.
    ///
    /// # Example
    /// ```
    ///  use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_auth_url("https://example.com/authorize");
    /// ```
    pub fn set_auth_url(&mut self, auth_url: &str) -> &mut AuthFlow {
        self.set_config("AUTH_URL", auth_url)
    }

    /// Set the token url of a request for OAuth
    ///
    /// # Example
    /// ```
    ///  use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_token_url("https://example.com/token");
    /// ```
    pub fn set_token_url(&mut self, token_url: &str) -> &mut AuthFlow {
        self.set_config("TOKEN_URL", token_url)
    }

    /// Set the redirect uri of a request
    ///
    /// # Example
    /// ```
    ///  use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_redirect_uri("https://localhost:8888/redirect");
    /// ```
    pub fn set_redirect_uri(&mut self, redirect_uri: &str) -> &mut AuthFlow {
        self.set_config("REDIRECT_URI", redirect_uri)
    }

    /// Set the response type of a request:
    ///     1. Code Flow: 'code'
    ///     2. Token Flow: 'token'
    ///
    /// # Example
    /// ```
    ///  use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_response_type("code");
    /// ```
    pub fn set_response_type(&mut self, response_type: &str) -> &mut AuthFlow {
        self.set_config("RESPONSE_TYPE", response_type)
    }

    /// Set the code of a request - returned from log in and authorization
    pub fn set_access_code(&mut self, code: &str) -> &mut AuthFlow {
        self.set_config("CODE", code)
    }

    /// Set the refresh token of a request
    pub fn set_refresh(&mut self, code: &str) -> &mut AuthFlow {
        self.set_config("REFRESH_TOKEN", code)
    }

    /// Set the token of a request
    pub fn set_access_token(&mut self, token: &str) -> &mut AuthFlow {
        self.set_config("ACCESS_TOKEN", token)
    }

    pub fn set_access_token_struct(&mut self, access_token: AccessToken) {
        self.access_token = Some(Box::new(access_token));
    }

    /// Add a scope' for the OAuth URL.
    ///
    /// # Example
    /// ```
    /// use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(false);
    /// auth_flow.add_scope("Read");
    /// // or
    /// auth_flow.add_scope("Read")
    ///     .add_scope("Write")
    ///     .add_scope("ReadWrite.All");
    /// ```
    pub fn add_scope(&mut self, scope: &str) -> &mut AuthFlow {
        self.scopes.push(scope.to_string());
        self
    }

    pub fn set_response_mode(&mut self, response_mode: &str) -> &mut AuthFlow {
        self.set_config("RESPONSE_MODE", response_mode)
    }

    /// Set the state of a request
    pub fn set_state(&mut self, state: &str) -> &mut AuthFlow {
        self.set_config("STATE", state)
    }

    pub fn get_client_id(&self) -> Option<&String> {
        self.params.get("CLIENT_ID").clone()
    }

    pub fn get_client_secret(&self) -> Option<&String> {
        self.params.get("CLIENT_SECRET").clone()
    }

    pub fn get_auth_url(&self) -> Option<&String> {
        self.params.get("AUTH_URL").clone()
    }

    pub fn get_token_url(&self) -> Option<&String> {
        self.params.get("TOKEN_URL").clone()
    }

    pub fn get_redirect_uri(&self) -> Option<&String> {
        self.params.get("REDIRECT_URI").clone()
    }

    pub fn get_access_code(&self) -> Option<&String> {
        self.params.get("CODE").clone()
    }

    pub fn get_access_token(&self) -> Option<&String> {
        self.params.get("ACCESS_TOKEN").clone()
    }

    pub fn get_refresh_token(&self) -> Option<&String> {
        self.params.get("REFRESH_TOKEN").clone()
    }

    pub fn get_scopes(&self) -> Option<&Vec<String>> {
        Some(&self.scopes)
    }

    pub fn get_access_token_struct(self) -> Option<Box<AccessToken>> {
        self.access_token.clone()
    }

    fn set_config(&mut self, config_key: &str, config_value: &str) -> &mut AuthFlow {
        if !self.params.contains_key(config_key) || self.allow_reset {
            self.params
                .insert(config_key.to_string(), config_value.to_string());
        } else {
            AuthFlow::print_reset_error(config_key);
        }
        self
    }

    /// By default, once a value has a value has been set (token, auth_url, etc.)
    /// the value cannot be changed unless the caller explicitly says to allow
    /// change.
    ///
    /// Give the bool true as the argument to allow changing values. This is
    /// permanent until the caller explicitly sets allow_change to false.
    ///
    /// # Example
    /// ```
    /// use rust_onedrive::flow::v1::AuthFlow;
    ///
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow.set_client_id("client_id");
    /// // attempting to change client id would not work here.
    ///
    /// // Allow changing the authorization flow parameters before or after
    /// // a request
    /// auth_flow.allow_reset(true);
    ///
    /// // Client id can be changed now.
    /// auth_flow.set_client_id("new_client_id");
    ///
    /// // Set the allow reset back to false for safety
    /// auth_flow.allow_reset(false);
    ///  ```
    pub fn allow_reset(&mut self, allow_reset: bool) {
        self.allow_reset = allow_reset;
    }

    fn print_reset_error(config_to_reset: &str) {
        println!(
            "\nERROR:\n{} has already been set, call allow_reset(true) to change {}\n",
            config_to_reset, config_to_reset
        );
    }

    // Join the scopes when manually setting them versus
    // using the default url: https://graph.microsoft.com/.default
    // default_auth must be set to false.
    pub fn join_scopes(&mut self) -> String {
        self.scopes.join(" ")
    }

    pub fn get_default_auth_setting(&self) -> &AccountType {
        &self.auth_type
    }

    pub fn use_default_auth_url(&mut self, auth_version: AccountType) -> &mut AuthFlow {
        match auth_version {
            AccountType::Account => {
                if !self.allow_reset {
                    self.allow_reset(true);
                    self.default_auth = true;
                    self.auth_type = AccountType::Account;
                    self.set_auth_url(AuthUrl::AccountAuth.as_str());
                    self.set_token_url(AuthUrl::AccountToken.as_str());
                    self.allow_reset(false);
                } else {
                    self.auth_type = AccountType::Account;
                    self.set_auth_url(AuthUrl::AccountAuth.as_str());
                    self.set_token_url(AuthUrl::AccountToken.as_str());
                }
            }
            AccountType::Graph => {
                if !self.allow_reset {
                    self.allow_reset(true);
                    self.default_auth = true;
                    self.auth_type = AccountType::Graph;
                    self.set_auth_url(AuthUrl::GraphAuth.as_str());
                    self.set_token_url(AuthUrl::GraphToken.as_str());
                    self.allow_reset(false);
                } else {
                    self.auth_type = AccountType::Graph;
                    self.set_auth_url(AuthUrl::GraphAuth.as_str());
                    self.set_token_url(AuthUrl::GraphToken.as_str());
                }
            }
        };
        self
    }

    /// Token flow and code flow are the same except for the response type.
    /// Token flow uses 'token' and code flow uses 'code'
    ///
    /// All flows must go through a browser or browser control.
    ///
    /// Token Flow
    ///     GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    ///     client_id={client_id}
    ///     &scope={scope}
    ///     &response_type=code
    ///     &redirect_uri={redirect_uri}
    ///
    /// Code Flow
    ///     GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    ///     client_id={client_id}
    ///     &scope={scope}
    ///     &response_type=token
    ///     &redirect_uri={redirect_uri}
    pub fn build(&mut self, to_build: FlowType) -> Option<String> {
        match to_build {
            FlowType::AuthorizeTokenFlow => Some(self.build_auth(to_build)),
            FlowType::AuthorizeCodeFlow => Some(self.build_auth(to_build)),
            FlowType::GrantTypeAuthCode => Some(
                self.build_grant_request(to_build)
                    .expect("Could not build access token body"),
            ),
            FlowType::GrantTypeRefreshToken => Some(
                self.build_grant_request(to_build)
                    .expect("Could not build refresh token body"),
            ),
        }
    }

    /// Access Tokens and Refresh Access Tokens
    ///
    /// Access Tokens
    ///
    /// POST https://login.microsoftonline.com/common/oauth2/v2.0/token
    /// Content-Type: application/x-www-form-urlencoded
    ///
    /// Body of request:
    ///     client_id={client_id}
    ///     &redirect_uri={redirect_uri}
    ///     &client_secret={client_secret}
    ///     &code={code}
    ///     &grant_type=authorization_code
    ///
    /// Refresh Access Tokens
    ///
    /// POST https://login.microsoftonline.com/common/oauth2/v2.0/token
    /// Content-Type: application/x-www-form-urlencoded
    ///
    /// Body of Request:
    ///     client_id={client_id}
    ///     &redirect_uri={redirect_uri}8
    ///     &client_secret={client_secret}
    ///     &refresh_token={refresh_token}
    ///     &grant_type=refresh_token
    pub fn build_grant_request(
        &mut self,
        grant_type: FlowType,
    ) -> result::Result<String, io::Error> {
        let req_type = match grant_type {
            FlowType::GrantTypeAuthCode => FlowType::GrantTypeAuthCode.as_str(),
            FlowType::GrantTypeRefreshToken => FlowType::GrantTypeRefreshToken.as_str(),
            FlowType::AuthorizeTokenFlow => {
                panic!(FlowErrorType::match_error_type(FlowErrorType::RequiresGrantType).message)
            }
            FlowType::AuthorizeCodeFlow => {
                panic!(FlowErrorType::match_error_type(FlowErrorType::RequiresGrantType).message)
            }
        };

        let param_type = match grant_type {
            FlowType::GrantTypeAuthCode => "code",
            FlowType::GrantTypeRefreshToken => "refresh_token",
            FlowType::AuthorizeTokenFlow => {
                panic!(FlowErrorType::match_error_type(FlowErrorType::RequiresGrantType).message)
            }
            FlowType::AuthorizeCodeFlow => {
                panic!(FlowErrorType::match_error_type(FlowErrorType::RequiresGrantType).message)
            }
        };

        let encoded: String = form_urlencoded::Serializer::new(String::from(""))
            .append_pair(
                "client_id",
                self.params
                    .get("CLIENT_ID")
                    .expect("Couldn't set client_id")
                    .as_str(),
            )
            .append_pair(
                "redirect_uri",
                self.params
                    .get("REDIRECT_URI")
                    .expect("Couldn't set redirect_id")
                    .as_str(),
            )
            .append_pair(
                "client_secret",
                self.params
                    .get("CLIENT_SECRET")
                    .expect("Couldn't set client_secret")
                    .as_str(),
            )
            .append_pair(
                param_type,
                self.params
                    .get(&param_type.to_uppercase())
                    .unwrap()
                    .as_str(),
            )
            .append_pair("grant_type", req_type)
            .finish();
        Ok(encoded.to_string())
    }

    /// Build the request url for authorization. The type of request depends
    /// upon the FlowType given as an argument.
    ///
    ///
    /// TOKEN FLOW = FlowType::
    ///     GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    ///     client_id={client_id}
    ///     &scope={scope}
    ///     &response_type=code
    ///     &redirect_uri={redirect_uri}
    ///
    /// CODE FLOW
    ///     GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    ///     client_id={client_id}
    ///     &scope={scope}
    ///     &response_type=token
    ///     &redirect_uri={redirect_uri}
    pub fn build_auth(&mut self, flow_type: FlowType) -> String {
        if self.default_auth {
            self.build_default_auth(flow_type)
        } else {
            let mut encoded = OauthUrlBuilder::new(true);
            encoded
                .scheme("")
                .host(self.params["AUTH_URL"].as_str())
                .path("");
            encoded.query(self.build_query(flow_type).as_str());
            encoded.build()
        }
    }

    fn build_default_auth(&mut self, flow_type: FlowType) -> String {
        match self.auth_type {
            AccountType::Account => {
                let mut encoded = OauthUrlBuilder::new(true);
                encoded
                    .scheme("")
                    .host(AuthUrl::AccountAuth.as_str())
                    .path("")
                    .query(self.build_query(flow_type).as_str());
                encoded.build()
            }
            AccountType::Graph => {
                let mut encoded = OauthUrlBuilder::new(true);
                encoded
                    .scheme("")
                    .host(AuthUrl::GraphAuth.as_str())
                    .path("")
                    .query(self.build_query(flow_type).as_str());
                encoded.build()
            }
        }
    }

    fn build_query(&mut self, flow_type: FlowType) -> String {
        if self.default_scope {
            let mut query = String::from("client_id=");
            query.push_str(self.params["CLIENT_ID"].as_str());
            query.push_str("&scope=https://graph.microsoft.com/.default");
            query.push_str("&response_type=");
            query.push_str(flow_type.as_str());
            query.push_str("&redirect_uri=");
            query.push_str(self.params["REDIRECT_URI"].as_str());
            query
        } else {
            let mut query = String::from("client_id=");
            query.push_str(self.params["CLIENT_ID"].as_str());
            query.push_str("&scope=");
            query.push_str(self.scopes.join(" ").as_str());
            query.push_str("&response_type=");
            query.push_str(flow_type.as_str());
            query.push_str("&redirect_uri=");
            query.push_str(self.params["REDIRECT_URI"].as_str());
            query
        }
    }

    /// Build the request url for authorization using the form_urlencoded() method from the URL crate.
    /// This may or may not be different from build_auth(). The build_auth method sets the encode set
    /// based upon the Microsoft recommended set while this method uses the URL crate's encode set.
    /// The type of request depends upon the FlowType given as an argument.
    ///
    ///
    /// TOKEN FLOW = FlowType::
    ///     GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    ///     client_id={client_id}
    ///     &scope={scope}
    ///     &response_type=code
    ///     &redirect_uri={redirect_uri}
    ///
    /// CODE FLOW
    ///     GET https://login.microsoftonline.com/common/oauth2/v2.0/authorize?
    ///     client_id={client_id}
    ///     &scope={scope}
    ///     &response_type=token
    ///     &redirect_uri={redirect_uri}
    pub fn build_auth_using_form_urlencoded(&mut self, flow_type: FlowType) -> String {
        let mut auth_url = String::from(self.params["AUTH_URL"].as_str());
        let encoded: String = form_urlencoded::Serializer::new(String::from(""))
            .append_pair("client_id", &self.params["CLIENT_ID"].to_string())
            .append_pair("scope", "https://graph.microsoft.com/.default")
            .append_pair("response_type", flow_type.as_str())
            .append_pair("redirect_uri", &self.params["REDIRECT_URI"].to_string())
            .finish();

        auth_url.push_str(&encoded);
        auth_url
    }

    /// Open the browser to the authentication page. Once the user signs in the
    /// page will redirect to the url that was specified for redirect_url.
    ///
    /// Linux: There are a couple of options to open the browser with a url.
    ///     1. Use xdg-open to open the browser using Command:
    ///         Command::new("xdg-open").arg("url-to-open");
    ///     2. Use Command to call the firefox process directly:
    ///         .arg("firefox")
    ///         .arg("--new-window");
    ///
    /// On Linux it is much better to use xdg-open. Using the firefox command that is
    /// built in with the browser may result in rogue processes.
    pub fn browser_flow(&mut self) -> io::Result<()> {
        let auth_url = self.build(FlowType::AuthorizeCodeFlow).unwrap();
        let handle = thread::spawn(move || {
            let auth_to_string = auth_url.as_str();
            Command::new("xdg-open")
                .arg(auth_to_string)
                .spawn()
                .expect("Could not open browser");
        });

        // Make sure threads spawn and finish
        handle.join().unwrap();
        Ok(())
    }

    /// Request Access Tokens
    ///
    /// Builds the url and performs post request for access token.
    /// If successful, the access token String and the struct AccessToken
    /// will automatically be set (see example). This method requires
    /// the token_url and access_code to be set and valid for performing
    /// the post request.
    ///
    /// Don't confuse an access_code for an access_token. The access_code is
    /// used to perform the post request for an access_token. The access_code
    /// is retrieved from the url on a browser redirect when logging in to
    /// a Microsoft account.
    ///
    /// An access token request has the following requirements:
    ///
    /// POST https://login.live.com/oauth20_token.srf
    /// Content-Type: application/x-www-form-urlencoded
    ///
    /// Body of Request:
    ///     client_id={client_id}
    ///     &redirect_uri={redirect_uri}
    ///     &client_secret={client_secret}
    ///     &refresh_token={refresh_token}
    ///     &grant_type=refresh_token
    ///
    /// # Example
    /// ```rust,ignore
    /// let mut auth_flow = AuthFlow::new(true);
    /// auth_flow
    ///     .set_auth_url("https://login.live.com/oauth20_authorize.srf")
    ///     .set_client_id("<client id>")
    ///     .set_client_secret("<client secret>")
    ///     .set_redirect_uri("http://localhost:8000/redirect")
    ///     .set_token_url("https://login.live.com/oauth20_token.srf");
    ///
    /// // Send user to url for for access code
    /// // then set the access code for an AuthFlow struct
    /// auth_flow.set_access_token("<access code>");
    ///
    /// // Run request_access_token() which will automatically set the
    /// // AccessToken struct in AuthFlow.access_token
    /// auth_flow.request_access_token()?;
    ///
    /// // Get the AccessToken struct
    /// let access_token: AccessToken = auth_flow.get_access_token_struct()?;
    /// // Get the access token String from AccessToken
    /// println!("{:#?}", access_token.get_access_token());
    ///
    /// // or
    ///
    /// // Get only the access token string which is also set in AuthFlow
    /// // when calling request_access_token()
    /// println!("{:#?}", auth_flow.get_access_token());
    /// ```
    pub fn request_access_token(&mut self) -> &mut AuthFlow {
        let client = reqwest::Client::builder()
            .build()
            .expect("could not construct reqwest builder");
        let code_body = self
            .build(FlowType::GrantTypeAuthCode)
            .expect("Could not build with FlowType::GrantTypeAuthCode");
        let access_code = self.params.get("CODE").expect(
            "Could not find access token in HashMap. Ensure the value has been set correctly",
        );
        let access_token_url = self
            .params
            .get("TOKEN_URL")
            .expect("Could not find token_url in HashMap. Ensure the value has been set correctly");

        let mut res = client
            .post(access_token_url)
            .header(header::AUTHORIZATION, access_code.as_str())
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(code_body)
            .send()
            .expect("Error in sending access token post request");

        let json_str = res.text().expect(
            FlowErrorType::match_error_type(FlowErrorType::BadRequest)
                .message
                .as_str(),
        );
        let data = json::parse(&json_str.as_str()).expect(
            FlowErrorType::match_error_type(FlowErrorType::BadRequest)
                .message
                .as_str(),
        );
        let access_token_str = data["access_token"].as_str().expect(
            FlowErrorType::match_error_type(FlowErrorType::BadRequest)
                .message
                .as_str(),
        );

        self.set_access_token(&access_token_str);
        self.set_access_token_struct(AccessToken::new(
            data["token_type"]
                .as_str()
                .expect("could not convert token_type to str"),
            data["expires_in"]
                .as_u64()
                .expect("could not convert expires_in to u64"),
            data["scope"]
                .as_str()
                .expect("could not convert scope to str"),
            &access_token_str,
            // TODO: Fix request_access_token USER_ID
            // This should be set by the request but it currently is not known
            // whether it will come in the request or not and therefore may throw an error
            // if this does not comes in the request. Figure out what causes the graph API
            // to return with and without the user_id.
            "user_id",
        ));

        self
    }

    pub fn into_drive(&mut self) -> Drive {
        Drive::new(
            self.get_access_token().expect(
                FlowErrorType::match_error_type(FlowErrorType::MissingAccessCode)
                    .message
                    .as_str(),
            ),
        )
    }

    /// Check if the values have been set
    pub fn check(&self, flow_status: FlowStatus) -> bool {
        match flow_status {
            FlowStatus::AccessCode => self.params.contains_key("CODE"),
            FlowStatus::AccessToken => self.params.contains_key("ACCESS_TOKEN"),
        }
    }

    /// Writes the AuthFlow struct as a JSON file using serde_json.
    /// An AuthFlow struct can then be brought back in.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to a file and the file name itself.
    pub fn as_json_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        JsonFile::json_file(path, self)
    }

    /// Writes the AuthFlow struct as a JSON file using serde_json only if the path/file
    /// is not already created.
    /// # Arguments
    ///
    /// * `path` - Path to a file and the file name itself.
    pub fn as_new_json_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        JsonFile::new_json_file(path, self)
    }

    /// Get a Graph from a previously saved Graph as JSON
    ///
    /// # Arguments
    ///
    /// * `path` - Path to a file and the file name itself.
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<AuthFlow> {
        let auth_flow: AuthFlow =
            JsonFile::from_file(path).expect("Could not deserialize AuthFlow from file");
        Ok(auth_flow)
    }

    pub fn from_file_as_vec<P: AsRef<Path>>(path: P) -> io::Result<Vec<AuthFlow>> {
        let f = File::open(path)?;
        let buffer = BufReader::new(f);
        let mut graph_vec: Vec<AuthFlow> = Vec::new();

        for file_name in buffer.lines() {
            let graph = AuthFlow::from_file(&file_name?)?;
            graph_vec.push(graph);
        }

        Ok(graph_vec)
    }
}
