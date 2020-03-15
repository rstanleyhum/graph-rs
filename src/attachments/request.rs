use crate::client::Graph;
use crate::http::{GraphResponse, IntoResponse};
use crate::types::collection::Collection;
use crate::types::content::Content;
use reqwest::Method;

register_client!(AttachmentRequest,);

impl<'a> AttachmentRequest<'a> {
    get!( | get, serde_json::Value => "attachments/{{id}}" );
    get!( | content, GraphResponse<Content> => "attachments/{{id}}/$value" );
    delete!( | delete, GraphResponse<Content> => "attachments/{{id}}" );

    pub fn calendars(&'a self) -> CalendarAttachmentRequest<'a> {
        CalendarAttachmentRequest::new(self.client)
    }

    pub fn calendar_groups(&'a self) -> CalendarGroupAttachmentRequest<'a> {
        CalendarGroupAttachmentRequest::new(self.client)
    }

    pub fn mail_folder(&'a self) -> MailFolderMessageAttachmentRequest<'a> {
        MailFolderMessageAttachmentRequest::new(self.client)
    }

    pub fn messages(&'a self) -> MailMessageAttachmentRequest<'a> {
        MailMessageAttachmentRequest::new(self.client)
    }

    pub fn thread_posts(&'a self) -> ThreadPostAttachmentRequest<'a> {
        ThreadPostAttachmentRequest::new(self.client)
    }

    pub fn conversation_posts(&'a self) -> ThreadConvoPostAttachmentRequest<'a> {
        ThreadConvoPostAttachmentRequest::new(self.client)
    }
}

register_client!(CalendarAttachmentRequest,);

impl<'a> CalendarAttachmentRequest<'a> {
    get!( || get_default, serde_json::Value => "events/{{id}}/attachments/{{id}}" );
    get!( || default_content, GraphResponse<Content> => "events/{{id}}/attachments/{{id}}/$value" );
    delete!( || delete_default, GraphResponse<Content> => "events/{{id}}/attachments/{{id}}" );
    get!( ||| get, serde_json::Value => "calendar/{{id}}/events/{{id2}}/attachments/{{id3}}" );
    get!( ||| content, GraphResponse<Content> => "calendar/{{id}}/events/{{id2}}/attachments/{{id3}}/$value" );
    delete!( ||| delete, GraphResponse<Content> => "calendar/{{id}}/events/{{id2}}/attachments/{{id3}}" );
}

register_client!(CalendarGroupAttachmentRequest,);

impl<'a> CalendarGroupAttachmentRequest<'a> {
    get!( ||| get_default, serde_json::Value => "calendargroup/calendars/{{id}}/events/{{id2}}/attachments/{{id3}}" );
    get!( ||| default_content, GraphResponse<Content> => "calendargroup/calendars/{{id}}/events/{{id2}}/attachments/{{id3}}/$value" );
    delete!( ||| delete_default, GraphResponse<Content> => "calendargroup/calendars/{{id}}/events/{{id2}}/attachments/{{id3}}" );
    get!( |||| get, serde_json::Value => "calendargroups/{{id}}/calendars/{{id2}}/events/{{id3}}/attachments/{{id4}}" );
    get!( |||| content, GraphResponse<Content> => "calendargroups/{{id}}/calendars/{{id2}}/events/{{id3}}/attachments/{{id4}}/$value" );
    delete!( |||| delete, GraphResponse<Content> => "calendargroups/{{id}}/calendars/{{id2}}/events/{{id3}}/attachments/{{id4}}" );
}

register_client!(MailMessageAttachmentRequest,);

impl<'a> MailMessageAttachmentRequest<'a> {
    get!( || get, serde_json::Value => "messages/{{id}}/attachments/{{id2}}" );
    post!( [ | add, serde_json::Value => "messages/{{id}}/attachments" ] );
    get!( || content, GraphResponse<Content> => "messages/{{id}}/attachments/{{id2}}/$value" );
    delete!( || delete, GraphResponse<Content> => "messages/{{id}}/attachments/{{id2}}" );

    pub fn mail_folder(&'a self) -> MailFolderMessageAttachmentRequest<'a> {
        MailFolderMessageAttachmentRequest::new(self.client)
    }
}

register_client!(MailFolderMessageAttachmentRequest,);

impl<'a> MailFolderMessageAttachmentRequest<'a> {
    get!( ||| get, serde_json::Value => "mailFolders/{{id}}/messages/{{id2}}/attachments/{{id3}}" );
    get!( ||| content, GraphResponse<Content> => "mailFolders/{{id}}/messages/{{id2}}/attachments/{{id3}}/$value" );
    post!( [ || add, serde_json::Value => "mailFolders/{{id}}/messages/{{id2}}/attachments" ] );
    delete!( ||| delete, GraphResponse<Content> => "mailFolders/{{id}}/messages/{{id2}}/attachments/{{id3}}" );

    fn render_child_folder_path<S: AsRef<str>>(
        &'a self,
        mail_folder_id: S,
        child_folders: &[&str],
        message_id: S,
        attachment_id: S,
        content: bool,
    ) {
        let vec: Vec<String> = child_folders
            .iter()
            .map(|s| format!("childFolders/{}/", s))
            .collect();

        let path = {
            if content {
                format!(
                    "mailFolders/{{{{id}}}}/{}/messages/{{{{id2}}}}/attachments/{{{{id3}}}}/$value",
                    vec.join("")
                )
            } else {
                format!(
                    "mailFolders/{{{{id}}}}/{}/messages/{{{{id2}}}}/attachments/{{{{id3}}}}",
                    vec.join("")
                )
            }
        };

        render_path!(
            self.client,
            path.as_str(),
            &serde_json::json!({
             "id": mail_folder_id.as_ref(),
             "id2": message_id.as_ref(),
             "id3": attachment_id.as_ref(),
            })
        );
    }

    pub fn child_folder<S: AsRef<str>>(
        &'a self,
        mail_folder_id: S,
        child_folders: &[&str],
        message_id: S,
        attachment_id: S,
    ) -> IntoResponse<'a, serde_json::Value> {
        self.client.builder().set_method(Method::GET);
        self.render_child_folder_path(
            mail_folder_id,
            child_folders,
            message_id,
            attachment_id,
            false,
        );
        IntoResponse::new(self.client)
    }

    pub fn child_folder_content<S: AsRef<str>>(
        &'a self,
        mail_folder_id: S,
        child_folders: &[&str],
        message_id: S,
        attachment_id: S,
    ) -> IntoResponse<'a, serde_json::Value> {
        self.client.builder().set_method(Method::GET);
        self.render_child_folder_path(
            mail_folder_id,
            child_folders,
            message_id,
            attachment_id,
            true,
        );
        IntoResponse::new(self.client)
    }

    pub fn delete_child_folder<S: AsRef<str>>(
        &'a self,
        mail_folder_id: S,
        child_folders: &[&str],
        message_id: S,
        attachment_id: S,
    ) -> IntoResponse<'a, serde_json::Value> {
        self.client.builder().set_method(Method::DELETE);
        self.render_child_folder_path(
            mail_folder_id,
            child_folders,
            message_id,
            attachment_id,
            false,
        );
        IntoResponse::new(self.client)
    }
}

register_client!(ThreadPostAttachmentRequest,);

impl<'a> ThreadPostAttachmentRequest<'a> {
    get!( || list, Collection<serde_json::Value> => "threads/{{id}}/posts/{{id2}}/attachments" );
    get!( ||| get, serde_json::Value => "threads/{{id}}/posts/{{id2}}/attachments/{{id3}}" );
    get!( ||| content, GraphResponse<Content> => "threads/{{id}}/posts/{{id2}}/attachments/{{id3}}/$value" );
    delete!( ||| delete, GraphResponse<Content> => "threads/{{id}}/posts/{{id2}}/attachments/{{id3}}" );
}

register_client!(ThreadConvoPostAttachmentRequest,);

impl<'a> ThreadConvoPostAttachmentRequest<'a> {
    get!( ||| list, Collection<serde_json::Value> => "conversations/{{id}}/threads/{{id2}}/posts/{{id3}}/attachments" );
    get!( |||| get, serde_json::Value => "conversations/{{id}}/threads/{{id2}}/posts/{{id3}}/attachments/{{id4}}" );
    get!( |||| content, GraphResponse<Content> => "conversations/{{id}}/threads/{{id2}}/posts/{{id3}}/attachments/{{id4}}/$value" );
    delete!( |||| delete, GraphResponse<Content> => "conversations/{{id}}/threads/{{id2}}/posts/{{id3}}/attachments/{{id4}}" );
}
