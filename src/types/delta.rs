use crate::error::GraphFailure;
use crate::http::GraphResponse;
use crate::types::content::Content;
use graph_error::GraphResult;
use reqwest::header::CONTENT_TYPE;
use serde::export::PhantomData;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

pub enum Delta<T> {
    Next(GraphResponse<T>),
    Done(Option<GraphFailure>),
}

pub trait NextLink<RHS = Self> {
    fn next_link(&self) -> Option<String>;
}

pub trait DeltaLink<RHS = Self> {
    fn delta_link(&self) -> Option<String>;

    fn delta<T: 'static + Send + NextLink>(&self, access_token: &str) -> Option<Receiver<Delta<T>>>
    where
        for<'de> T: serde::Deserialize<'de>,
    {
        let link = self.delta_link()?;
        let token = access_token.to_string();
        let (sender, receiver) = channel();
        let client = reqwest::Client::new();
        let response = client
            .get(link.as_str())
            .bearer_auth(token.as_str())
            .send()
            .map_err(GraphFailure::from);

        if let Err(err) = response {
            sender.send(Delta::Done(Some(err))).unwrap();
            return Some(receiver);
        }

        let mut res = response.unwrap();
        if let Some(err) = GraphFailure::from_response(&mut res) {
            sender.send(Delta::Done(Some(err))).unwrap();
            return Some(receiver);
        }

        let next: GraphResult<T> = res.json().map_err(GraphFailure::from);
        if let Err(err) = next {
            sender.send(Delta::Done(Some(err))).unwrap();
            return Some(receiver);
        }

        let value: T = next.unwrap();
        let mut next_link = value.next_link();
        sender
            .send(Delta::Next(GraphResponse::new(res, value)))
            .unwrap();

        thread::spawn(move || {
            while let Some(next) = next_link {
                let res = client
                    .get(next.as_str())
                    .header(CONTENT_TYPE, "application/json")
                    .bearer_auth(token.as_str())
                    .send()
                    .map_err(GraphFailure::from);

                if let Err(err) = res {
                    next_link = None;
                    sender.send(Delta::Done(Some(err))).unwrap();
                } else {
                    let mut response = res.unwrap();
                    if let Some(err) = GraphFailure::from_response(&mut response) {
                        next_link = None;
                        sender.send(Delta::Done(Some(err))).unwrap();
                    }

                    let value_res: GraphResult<T> = response.json().map_err(GraphFailure::from);
                    if let Err(err) = value_res {
                        next_link = None;
                        sender.send(Delta::Done(Some(err))).unwrap();
                    } else {
                        let value = value_res.unwrap();
                        next_link = value.next_link();
                        sender
                            .send(Delta::Next(GraphResponse::new(response, value)))
                            .unwrap();
                    }
                }
            }
            sender.send(Delta::Done(None)).unwrap();
        });

        Some(receiver)
    }
}

pub trait MetadataLink<RHS = Self> {
    fn metadata_link(&self) -> Option<String>;

    fn metadata(&self) -> Option<GraphResult<GraphResponse<Content>>> {
        let link = self.metadata_link()?;
        let client = reqwest::Client::new();
        let response = client.get(link.as_str()).send();

        if let Err(e) = response {
            Some(Err(GraphFailure::from(e)))
        } else if let Ok(mut res) = response {
            if let Ok(text) = res.text() {
                Some(Ok(GraphResponse::new(res, Content::new(text.as_str()))))
            } else {
                Some(Ok(GraphResponse::new(res, Content::new(""))))
            }
        } else {
            None
        }
    }
}

#[derive(Default)]
pub struct DeltaRequest<T> {
    phantom: PhantomData<T>,
}

impl NextLink for serde_json::Value {
    fn next_link(&self) -> Option<String> {
        self["@odata.nextLink"].as_str().map(|s| s.to_string())
    }
}

impl DeltaLink for serde_json::Value {
    fn delta_link(&self) -> Option<String> {
        self["@odata.deltaLink"].as_str().map(|s| s.to_string())
    }
}

impl MetadataLink for serde_json::Value {
    fn metadata_link(&self) -> Option<String> {
        self["@odata.context"].as_str().map(|s| s.to_string())
    }
}
