use crate::client::Graph;
use crate::http::{GraphResponse, IntoResponse};
use crate::types::{collection::Collection, content::Content, delta::DeltaRequest};
use handlebars::*;
use reqwest::Method;

register_client!(
    ContactsRequest,
    ct => "contacts",
    cf => "contactfolders",
);

impl<'a> ContactsRequest<'a> {
    get!( delta, DeltaRequest<Collection<serde_json::Value>> => "{{ct}}/delta" );
    get!( list, Collection<serde_json::Value> => "{{ct}}" );
    get!( | get, serde_json::Value => "{{ct}}/{{id}}" );
    post!( [ create, serde_json::Value => "{{ct}}" ] );
    patch!( [ | update, serde_json::Value => "{{ct}}/{{id}}" ] );
    delete!( | delete, GraphResponse<Content> => "{{ct}}/{{id}}" );

    pub fn contacts_folder(&'a self) -> ContactsFolderRequest<'a> {
        ContactsFolderRequest::new(self.client)
    }
}

register_client!(ContactsFolderRequest,);

impl<'a> ContactsFolderRequest<'a> {
    get!( delta, DeltaRequest<Collection<serde_json::Value>> => "{{cf}}/delta" );
    get!( | get, serde_json::Value => "{{cf}}/{{id}}" );
    get!( | list_child_folders, Collection<serde_json::Value> => "{{cf}}/{{id}}/childFolders" );
    post!( [ | create_child_folder, serde_json::Value => "{{cf}}/{{id}}/childFolders" ] );
    patch!( [ | update, serde_json::Value => "{{cf}}/{{id}}" ] );
    delete!( | delete, GraphResponse<Content> => "{{cf}}/{{id}}" );

    pub fn contacts(&'a self) -> ContactsFolderContactsRequest<'a> {
        ContactsFolderContactsRequest::new(self.client)
    }
}

register_client!(ContactsFolderContactsRequest,);

impl<'a> ContactsFolderContactsRequest<'a> {
    get!( | delta, DeltaRequest<Collection<serde_json::Value>> => "{{cf}}/{{id}}/{{ct}}/delta" );
    get!( | list, Collection<serde_json::Value> => "{{cf}}/{{id}}/{{ct}}" );
    post!( [ | create, serde_json::Value => "{{cf}}/{{id}}" ] );
    delete!( || delete, GraphResponse<Content> => "{{cf}}/{{id}}/{{ct}}/{{id2}}" );
}
