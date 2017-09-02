mod entities {

    use chrono::prelude::*;
    use modules::*;

    // Where do we request updates to be sent to
    // and from where do we connect to link accounts
    enum Connector {
        Matrix,
    }

    // Where do we retrieve updates from
    enum Adapter {
        #[cfg(feature = "linux-org-ru")]
        LinuxOrgRu,
    }

    // Common trait that both Connectors and Adapters possess
    trait Connectable {
        fn connect(&self);
    }

    trait Pollable: Connectable {
        fn poll<T>(&self) -> Vec<T>
        where
            T: ToString;
    }

    trait Notifiable: Connectable {
        fn send(&self, comment: UserComment);
    }

    impl Connectable for Connector {
        fn connect(&self) {
            match self {
                Matrix => {}
            }
        }
    }

    impl Notifiable for Connector {
        fn send(&self, comment: UserComment) {
            match self {
                Matrix => {}
            }
        }
    }

    // User info struct, which provides a link between Connector and Adapter
    // UserInfo struct instances are meant to be alive almost the same amount of time
    // the application is running.
    struct UserInfo {
        userId: i64,            // internal user ID as saved in DB, mostly not used
        userName: String,       // user name as provided by Connector
        linkedUserName: String, // linked user name, as requested from Adapter
        connector: Connector,   // Connector itself, most of the time it's in `connected` state
        adapter: Adapter,       // Adapter itself, most of the time it's in `connected` state
        lastUpdate: DateTime<Utc>, // Last time update was queried for this instance
    }
}