use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectedTicketsResponse {
    #[serde(default)]
    pub tickets: Vec<ConnectedTicket>,
    #[serde(rename = "allTicketsQuery")]
    pub all_tickets_query: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectedTicket {
    pub key: String,
    pub id: String,
    pub title: String,
    pub reporter: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub status: Option<TicketStatus>,
    #[serde(rename = "type")]
    pub issue_type: Option<TicketType>,
    pub priority: Option<TicketPriority>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketStatus {
    pub name: String,
    #[serde(rename = "colorName")]
    pub color_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketType {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketPriority {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_connected_tickets_response() {
        let json = r#"{
            "tickets": [
                {
                    "key": "PROJ-42",
                    "id": "10968",
                    "title": "VPN access not working",
                    "reporter": "abc123",
                    "created": "2026-02-17T18:31:56.953Z",
                    "updated": "2026-03-22T18:59:23.333Z",
                    "status": { "name": "In Progress", "colorName": "yellow" },
                    "type": { "name": "Service Request" },
                    "priority": { "name": "High" }
                }
            ],
            "allTicketsQuery": "issueFunction in assetsObject(\"objectId = 88\")"
        }"#;
        let resp: ConnectedTicketsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.tickets.len(), 1);
        assert_eq!(resp.tickets[0].key, "PROJ-42");
        assert_eq!(resp.tickets[0].title, "VPN access not working");
        assert_eq!(resp.tickets[0].status.as_ref().unwrap().name, "In Progress");
        assert!(resp.all_tickets_query.is_some());
    }

    #[test]
    fn deserialize_empty_tickets() {
        let json = r#"{ "tickets": [] }"#;
        let resp: ConnectedTicketsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.tickets.is_empty());
        assert!(resp.all_tickets_query.is_none());
    }
}
