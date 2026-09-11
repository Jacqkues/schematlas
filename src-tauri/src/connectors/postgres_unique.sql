SELECT n.nspname::text, t.relname::text, i.indexrelid::text, a.attname::text
FROM pg_index i JOIN pg_class t ON t.oid=i.indrelid
JOIN pg_namespace n ON n.oid=t.relnamespace
CROSS JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum, ordinal)
JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=k.attnum
WHERE i.indisunique AND i.indisvalid AND i.indpred IS NULL AND i.indexprs IS NULL
AND k.ordinal <= i.indnkeyatts AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema'
ORDER BY n.nspname,t.relname,i.indexrelid,k.ordinal
