create table if not exists "metadata" (
    "key" text primary key,
    "value" text not null
);

create table if not exists "emojis" (
    "host" text,
    "hash" text,
    "name" text,
    "category" text,
    "aliases" json,
    "license" text,
    "image_url" text,
    "etag" text,
    "last_modified" text
);

create index if not exists "ix_hash" on "emojis" ("hash");

create unique index if not exists "ix_unique" on "emojis" (
    "host", "name"
);


create table if not exists "hash_parts" (
    "hash" text,
    "position" integer,
    "part" text
);

create unique index if not exists "ix_hash_part" on "hash_parts" ("position", "part", "hash");

create table if not exists "servers" (
    "id" integer primary key,
    "url" text not null unique,
    "enabled" integer not null default 1,
    "last_fetched_at" text
);
