SELECT sn.nspname::text, sc.relname::text, sa.attname::text, tn.nspname::text, tc.relname::text, ta.attname::text, fk.conname::text
FROM pg_constraint fk JOIN pg_class sc ON sc.oid=fk.conrelid JOIN pg_namespace sn ON sn.oid=sc.relnamespace
JOIN pg_class tc ON tc.oid=fk.confrelid JOIN pg_namespace tn ON tn.oid=tc.relnamespace
CROSS JOIN LATERAL unnest(fk.conkey, fk.confkey) AS pair(src,dst)
JOIN pg_attribute sa ON sa.attrelid=sc.oid AND sa.attnum=pair.src
JOIN pg_attribute ta ON ta.attrelid=tc.oid AND ta.attnum=pair.dst
WHERE fk.contype='f' AND sn.nspname NOT LIKE 'pg_%' AND sn.nspname<>'information_schema'
ORDER BY sn.nspname,sc.relname,fk.conname,sa.attnum
