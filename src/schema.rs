table! {
    history (id) {
        id -> Integer,
        username -> Text,
        device -> Text,
        command -> Text,
        qtype -> Text,
        rejected -> Integer,
        timestamp -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Integer,
        name -> Text,
        password -> Text,
        groups -> Text,
    }
}

// meta table just for stat queries
table! {
    stat_entrys (label) {
        label -> Text,
        counter -> Integer,
    }
}

allow_tables_to_appear_in_same_query!(
    history,
    users,
);
