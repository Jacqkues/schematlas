---
title: Databases and APIs
description: Connect a database to map its live schema, or import an OpenAPI definition.
---

## Connect a database

Choose **Connect database** in the explorer, pick the engine, and enter the connection as fields or as a connection string. Schematlas inspects the catalog — tables, views, columns, keys, foreign keys and unique indexes — and draws the schema.

| Engine | Example connection |
| --- | --- |
| PostgreSQL | `postgresql://user:password@host:5432/database?sslmode=require` |
| MySQL, MariaDB | `mysql://user:password@host:3306/database?ssl-mode=REQUIRED` |
| SQLite | A database file, opened read-only |
| SQL Server | `Server=tcp:host,1433;Database=app;User ID=user;Password=…;Encrypt=true` |

Only metadata is read. The connection string stays in memory: it is never saved or logged, so after a restart you reconnect before refreshing. The schema map itself is saved and stays browsable offline.

## The schema canvas

- Search tables, filter schemas, and select a table to highlight its relationships.
- Cardinality comes from foreign keys, nullability and unique indexes.
- Organize tables in named, colored **groups**, and run automatic layout.

To share a schema with your team, save it to the repository as [DBML](/docs/dbml).

## Query with your agent

With a database connected, your agent can explore its schema and, after your approval of each exact statement, sample rows, read statistics, explain or run queries. Results are capped at 200 rows.

## Import an OpenAPI definition

**Import OpenAPI** reads OpenAPI 3 or Swagger 2, as JSON or YAML, up to 20 MB. Operations and models become a graph, laid out by tag and path. External references are reported, never fetched. Your agent can call documented operations, each after your approval.
