table! {
    groups (id) {
        id -> Integer,
        g_name -> Text,
        devices -> Text,
    }
}

table! {
    history (id) {
        id -> Integer,
        username -> Nullable<Text>,
        get_query -> Text,
        timestamp -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Integer,
        name -> Text,
        password -> Text,
        cookie -> Nullable<Text>,
        groups -> Text,
        wrong_attempts -> Nullable<Integer>,
    }
}

allow_tables_to_appear_in_same_query!(
    groups,
    history,
    users,
);
