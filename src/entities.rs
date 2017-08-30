mod entities {

use chrono::prelude::*;

    struct UserInfo {
        userId: i64,
        userName: String,
        linkedUserName: String,
        lastUpdate: DateTime<Utc>
    }
}