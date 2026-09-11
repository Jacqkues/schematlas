SELECT n.nspname::text, c.relname::text,
CASE WHEN c.relkind IN ('v','m') THEN 'view' ELSE 'table' END::text
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r','p','v','m','f') AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema'
ORDER BY n.nspname,c.relname
