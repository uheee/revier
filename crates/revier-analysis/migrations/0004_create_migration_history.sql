create table if not exists schema_migrations (
  version integer primary key,
  name text not null,
  checksum text not null,
  applied_at timestamp not null,
  kind text not null
);
