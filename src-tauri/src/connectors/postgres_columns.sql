SELECT n.nspname::text, c.relname::text, a.attname::text, format_type(a.atttypid,a.atttypmod)::text,
CASE WHEN a.attnotnull THEN 'NO' ELSE 'YES' END::text,
CASE WHEN EXISTS (SELECT 1 FROM pg_constraint p WHERE p.conrelid=c.oid AND p.contype='p' AND a.attnum=ANY(p.conkey)) THEN 'YES' ELSE 'NO' END::text,
pg_get_expr(d.adbin,d.adrelid)::text, col_description(c.oid,a.attnum)::text
FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace
LEFT JOIN pg_attrdef d ON d.adrelid=c.oid AND d.adnum=a.attnum
WHERE c.relkind IN ('r','p','v','m','f') AND a.attnum>0 AND NOT a.attisdropped
AND n.nspname NOT LIKE 'pg_%' AND n.nspname<>'information_schema'
ORDER BY n.nspname,c.relname,a.attnum
