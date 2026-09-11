SELECT s.name,o.name,c.name,
CAST(t.name + CASE
WHEN t.name IN ('varchar','char','varbinary','binary') THEN '(' + CASE WHEN c.max_length=-1 THEN 'max' ELSE CAST(c.max_length AS varchar(8)) END + ')'
WHEN t.name IN ('nvarchar','nchar') THEN '(' + CASE WHEN c.max_length=-1 THEN 'max' ELSE CAST(c.max_length/2 AS varchar(8)) END + ')'
WHEN t.name IN ('decimal','numeric') THEN '('+CAST(c.precision AS varchar(8))+','+CAST(c.scale AS varchar(8))+')'
ELSE '' END AS nvarchar(256)),
CASE WHEN c.is_nullable=1 THEN 'YES' ELSE 'NO' END,
CASE WHEN EXISTS (SELECT 1 FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id=i.object_id AND ic.index_id=i.index_id WHERE i.object_id=c.object_id AND i.is_primary_key=1 AND ic.column_id=c.column_id) THEN 'YES' ELSE 'NO' END,
d.definition,CAST(ep.value AS nvarchar(4000))
FROM sys.columns c JOIN sys.objects o ON o.object_id=c.object_id JOIN sys.schemas s ON s.schema_id=o.schema_id
JOIN sys.types t ON t.user_type_id=c.user_type_id
LEFT JOIN sys.default_constraints d ON d.object_id=c.default_object_id
LEFT JOIN sys.extended_properties ep ON ep.major_id=c.object_id AND ep.minor_id=c.column_id AND ep.name='MS_Description' AND ep.class=1
WHERE o.type IN ('U','V') AND o.is_ms_shipped=0 ORDER BY s.name,o.name,c.column_id
