-- Create table for UserInfo struct
create table user_info (
    id integer primary key autoincrement not null,
    upstream_type text not null,
    chat_id text not null,
    user_id text not null,
    adapter text not null,
    linked_user_id text not null,
    last_update datetime not null,
    verified boolean not null default 1
);

create index user_info_by_upstream on user_info(upstream_type);
create unique index user_infos_uniq on user_info(upstream_type, chat_id, user_id, adapter, linked_user_id);