//! The database schema, as an ordered list of SQL migrations.
//!
//! `PRAGMA user_version` records how many have been applied, so opening a
//! database only runs the ones it has not seen yet.

/// One schema change, applied at most once per database.
pub struct Migration {
    /// Identifies the migration in errors and logs.
    pub name: &'static str,
    /// The statements to run, as one batch inside a transaction.
    pub sql: &'static str,
}

/// How many of the first [`MIGRATIONS`] run with foreign keys on, so their
/// cascades fire as they did when they shipped. Every later one runs with
/// them off, as SQLite's way of rebuilding a table others refer to needs
/// (dropping the old table would otherwise run their cascades), and is
/// checked against them before it commits: one that leaves a row referring
/// to a row it removed fails, so it removes what refers to it itself.
pub const CASCADING_MIGRATIONS: usize = 48;

/// Every migration, in the order they are applied. Append only.
pub const MIGRATIONS: &[Migration] = &[
    // The first seven are the schema 68 earlier migrations had built by 22
    // October 2026, squashed into one per area. A database from before then
    // was moved onto them by setting its `user_version` to 7.

    // Paired clients and the key each one presents.
    Migration {
        name: "clients",
        sql: "
        CREATE TABLE clients (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            key_kind TEXT NOT NULL DEFAULT 'certificate',
            key TEXT NOT NULL,
            client_name TEXT NOT NULL
        );

        CREATE UNIQUE INDEX clients_live_key ON clients(key) WHERE deleted = FALSE;

        CREATE TRIGGER clients_deleted_own_nothing
        AFTER UPDATE OF deleted ON clients
        WHEN new.deleted
        BEGIN
            DELETE FROM intelligence_endpoints WHERE client_id = new.id;
            DELETE FROM connector_accounts WHERE client_id = new.id;
        END;",
    },
    // Workspaces, their documents and tasks, the metadata any row can carry,
    // and the search index they feed.
    Migration {
        name: "workspaces",
        sql: "
        CREATE TABLE workspaces (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            title TEXT NOT NULL,
            emoji TEXT
        );

        CREATE TRIGGER workspaces_metadata_ad AFTER DELETE ON workspaces
        BEGIN
            DELETE FROM metadata WHERE entity_kind = 'workspaces' AND entity_id = old.id;
        END;

        CREATE TABLE documents (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            document_type TEXT NOT NULL DEFAULT 'normal',
            title TEXT NOT NULL,
            contents TEXT NOT NULL,
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            journal_date TEXT CHECK (journal_date IS date(journal_date))
        );

        CREATE TRIGGER documents_search_ai AFTER INSERT ON documents WHEN new.deleted = 0
        BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('document', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title, new.contents);
        END;

        CREATE TRIGGER documents_search_ad AFTER DELETE ON documents
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'document' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'document' AND entity_id = old.id;
        END;

        CREATE TRIGGER documents_search_au
        AFTER UPDATE OF title, contents, deleted, workspace_id ON documents
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'document' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'document' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                SELECT 'document', new.id, new.workspace_id WHERE new.deleted = 0;
            INSERT INTO search_fts (rowid, title, contents)
                SELECT last_insert_rowid(), new.title, new.contents WHERE new.deleted = 0;
        END;

        CREATE TRIGGER documents_metadata_ad AFTER DELETE ON documents
        BEGIN
            DELETE FROM metadata WHERE entity_kind = 'documents' AND entity_id = old.id;
        END;

        CREATE TABLE task_folders (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE tasks (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
            task_folder_id INTEGER REFERENCES task_folders(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'incomplete',
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            priority INTEGER NOT NULL DEFAULT 0,
            description TEXT NOT NULL DEFAULT '',
            start_date TEXT CHECK (start_date IS date(start_date)),
            due_date TEXT CHECK (due_date IS date(due_date))
        );

        CREATE TRIGGER tasks_search_ad AFTER DELETE ON tasks
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id;
        END;

        CREATE TRIGGER tasks_metadata_ad AFTER DELETE ON tasks
        BEGIN
            DELETE FROM metadata WHERE entity_kind = 'tasks' AND entity_id = old.id;
        END;

        CREATE TRIGGER tasks_search_ai AFTER INSERT ON tasks WHEN new.deleted = 0
        BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('task', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title, new.description);
        END;

        CREATE TRIGGER tasks_search_au
        AFTER UPDATE OF title, description, deleted, workspace_id ON tasks
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                SELECT 'task', new.id, new.workspace_id WHERE new.deleted = 0;
            INSERT INTO search_fts (rowid, title, contents)
                SELECT last_insert_rowid(), new.title, new.description WHERE new.deleted = 0;
        END;

        CREATE TABLE metadata (
            id INTEGER PRIMARY KEY,
            entity_kind TEXT NOT NULL,
            entity_id INTEGER NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            UNIQUE (entity_kind, entity_id, key)
        );

        CREATE INDEX metadata_lookup ON metadata(entity_kind, key, value);

        CREATE TABLE search_entries (
            id INTEGER PRIMARY KEY,
            entity_kind TEXT NOT NULL,
            entity_id INTEGER NOT NULL,
            workspace_id INTEGER,
            UNIQUE (entity_kind, entity_id, workspace_id)
        );

        CREATE INDEX search_entries_workspace ON search_entries(workspace_id);

        CREATE VIRTUAL TABLE search_fts USING fts5(
                title, contents,
                tokenize='porter unicode61 remove_diacritics 2'
            );",
    },
    // Installed extensions, their grants, hooks and services, and the storage
    // scripts keep.
    Migration {
        name: "extensions",
        sql: "
        CREATE TABLE extensions (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            script TEXT NOT NULL,
            script_identifier TEXT NOT NULL UNIQUE,
            priority INTEGER NOT NULL DEFAULT 0,
            description TEXT NOT NULL DEFAULT '',
            author TEXT NOT NULL DEFAULT '',
            icon TEXT,
            announced_version TEXT
        );

        CREATE TRIGGER extensions_storage_ad AFTER DELETE ON extensions
        BEGIN
            DELETE FROM storage_entries WHERE sandbox = 'extension:' || old.script_identifier;
        END;

        CREATE TABLE extension_permissions (
            id INTEGER PRIMARY KEY,
            extension_id INTEGER NOT NULL REFERENCES extensions(id) ON DELETE CASCADE,
            permission TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT FALSE,
            UNIQUE (extension_id, permission)
        );

        CREATE TABLE extension_hooks (
            id INTEGER PRIMARY KEY,
            extension_id INTEGER NOT NULL REFERENCES extensions(id) ON DELETE CASCADE,
            hook TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT FALSE,
            UNIQUE (extension_id, hook)
        );

        CREATE TABLE extension_services (
            id INTEGER PRIMARY KEY,
            extension_id INTEGER NOT NULL UNIQUE REFERENCES extensions(id) ON DELETE CASCADE,
            description TEXT NOT NULL
        );

        CREATE TABLE extension_service_functions (
            id INTEGER PRIMARY KEY,
            service_id INTEGER NOT NULL
                REFERENCES extension_services(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            args TEXT,
            returns TEXT,
            read_only BOOLEAN NOT NULL,
            UNIQUE (service_id, name)
        );

        CREATE TABLE storage_entries (
            id INTEGER PRIMARY KEY,
            sandbox TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            UNIQUE (sandbox, key)
        );",
    },
    // Connector accounts, from an extension or a client's offer, the resources
    // an offer lists and the assets they import.
    Migration {
        name: "connectors",
        sql: "
        CREATE TABLE connector_accounts (
            id INTEGER PRIMARY KEY,
            extension_id INTEGER REFERENCES extensions(id) ON DELETE CASCADE,
            extension_identifier TEXT,
            identifier TEXT,
            client_id INTEGER REFERENCES clients(id) ON DELETE CASCADE,
            offer TEXT,
            service_title TEXT NOT NULL,
            account_unit TEXT NOT NULL,
            asset_kinds TEXT NOT NULL DEFAULT '[]',
            description TEXT,
            authorization TEXT,
            refresh TEXT,
            expires_at INTEGER,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK ((extension_id IS NOT NULL AND extension_identifier IS NOT NULL
                    AND identifier IS NOT NULL AND description IS NOT NULL
                    AND authorization IS NOT NULL AND client_id IS NULL AND offer IS NULL)
                OR (extension_id IS NULL AND extension_identifier IS NULL
                    AND identifier IS NULL AND description IS NULL AND authorization IS NULL
                    AND refresh IS NULL AND expires_at IS NULL
                    AND client_id IS NOT NULL AND offer IS NOT NULL)),
            UNIQUE (extension_id, identifier),
            UNIQUE (client_id, offer)
        );

        CREATE TRIGGER connector_accounts_live_client
        BEFORE INSERT ON connector_accounts
        WHEN new.client_id IS NOT NULL
        AND (SELECT deleted FROM clients WHERE id = new.client_id)
        BEGIN
            SELECT RAISE(ABORT, 'A deleted client owns no accounts.');
        END;

        CREATE TABLE edge_resources (
            id INTEGER PRIMARY KEY,
            account_id INTEGER NOT NULL
                REFERENCES connector_accounts(id) ON DELETE CASCADE,
            identifier TEXT NOT NULL,
            title TEXT NOT NULL,
            kind TEXT NOT NULL,
            offered_at INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (account_id, identifier)
        );

        CREATE TABLE assets (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            connector_account_id INTEGER NOT NULL
                REFERENCES connector_accounts(id) ON DELETE CASCADE,
            identifier TEXT NOT NULL,
            kind TEXT NOT NULL,
            kind_description TEXT NOT NULL,
            title TEXT NOT NULL,
            content_kind TEXT NOT NULL,
            contents TEXT NOT NULL,
            last_updated INTEGER NOT NULL,
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (workspace_id, connector_account_id, identifier)
        );

        CREATE INDEX assets_workspace_kind ON assets(workspace_id, kind);

        CREATE INDEX assets_account ON assets(connector_account_id);

        CREATE TRIGGER assets_search_ai AFTER INSERT ON assets
        BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('asset', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title,
                    iif(new.content_kind = 'markdown', new.contents, ''));
        END;

        CREATE TRIGGER assets_search_ad AFTER DELETE ON assets
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'asset' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'asset' AND entity_id = old.id;
        END;

        CREATE TRIGGER assets_search_au
        AFTER UPDATE OF title, contents, content_kind, workspace_id ON assets
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'asset' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'asset' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('asset', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title,
                    iif(new.content_kind = 'markdown', new.contents, ''));
        END;",
    },
    // Shared feeds, the articles they and saved links produce, and how each
    // workspace files them.
    Migration {
        name: "articles",
        sql: "
        CREATE TABLE feeds (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            url TEXT NOT NULL,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            full_page_policy TEXT NOT NULL DEFAULT 'whenExcerpt',
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );

        CREATE UNIQUE INDEX feeds_url ON feeds(url) WHERE deleted = FALSE;

        CREATE TABLE articles (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            feed_id INTEGER REFERENCES feeds(id) ON DELETE SET NULL,
            read BOOLEAN NOT NULL DEFAULT FALSE,
            url TEXT NOT NULL,
            title TEXT NOT NULL,
            author TEXT,
            published_at INTEGER NOT NULL DEFAULT 0,
            snapshot TEXT NOT NULL,
            contents TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK (contents = '' OR snapshot ->> '$.state' = 'captured')
        );

        CREATE INDEX articles_feed_url ON articles(feed_id, url) WHERE feed_id IS NOT NULL;

        CREATE INDEX articles_url ON articles(url) WHERE deleted = FALSE;

        CREATE TRIGGER articles_search_ad AFTER DELETE ON articles
        BEGIN
            DELETE FROM search_fts WHERE rowid IN
                (SELECT id FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id;
        END;

        CREATE TRIGGER articles_search_au AFTER UPDATE OF title, contents, deleted ON articles
        BEGIN
            DELETE FROM search_fts WHERE rowid IN
                (SELECT id FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                SELECT 'article', new.id, workspace_id FROM workspace_articles
                WHERE article_id = new.id AND new.deleted = 0;
            INSERT INTO search_fts (rowid, title, contents)
                SELECT id, new.title, new.contents FROM search_entries
                WHERE entity_kind = 'article' AND entity_id = new.id;
        END;

        CREATE TRIGGER articles_feed_corpus_ai AFTER INSERT ON articles
        WHEN new.feed_id IS NOT NULL AND new.deleted = 0
        BEGIN
            INSERT INTO feed_corpus_fts (rowid, title, author, contents)
                VALUES (new.id, new.title, new.author, new.contents);
        END;

        CREATE TRIGGER articles_feed_corpus_ad AFTER DELETE ON articles
        BEGIN
            DELETE FROM feed_corpus_fts WHERE rowid = old.id;
        END;

        CREATE TRIGGER articles_feed_corpus_au
        AFTER UPDATE OF title, author, contents, deleted, feed_id ON articles
        BEGIN
            DELETE FROM feed_corpus_fts WHERE rowid = old.id;
            INSERT INTO feed_corpus_fts (rowid, title, author, contents)
                SELECT new.id, new.title, new.author, new.contents
                WHERE new.feed_id IS NOT NULL AND new.deleted = 0;
        END;

        CREATE TABLE article_folders (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE workspace_articles (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
            article_folder_id INTEGER REFERENCES article_folders(id) ON DELETE SET NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (workspace_id, article_id)
        );

        CREATE INDEX workspace_articles_article ON workspace_articles(article_id);

        CREATE TRIGGER workspace_articles_search_ai AFTER INSERT ON workspace_articles
        BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                SELECT 'article', new.article_id, new.workspace_id FROM articles
                WHERE id = new.article_id AND deleted = 0;
            INSERT INTO search_fts (rowid, title, contents)
                SELECT entry.id, articles.title, articles.contents FROM search_entries AS entry
                INNER JOIN articles ON articles.id = entry.entity_id
                WHERE entry.entity_kind = 'article' AND entry.entity_id = new.article_id
                AND entry.workspace_id = new.workspace_id;
        END;

        CREATE TRIGGER workspace_articles_search_ad AFTER DELETE ON workspace_articles
        BEGIN
            DELETE FROM search_fts WHERE rowid IN
                (SELECT id FROM search_entries WHERE entity_kind = 'article'
                AND entity_id = old.article_id AND workspace_id = old.workspace_id);
            DELETE FROM search_entries WHERE entity_kind = 'article'
                AND entity_id = old.article_id AND workspace_id = old.workspace_id;
        END;

        CREATE VIRTUAL TABLE feed_corpus_fts USING fts5(title, author, contents,
            tokenize='porter unicode61 remove_diacritics 2');",
    },
    // Model endpoints and the catalog that prices them, the default per tier,
    // conversation threads and what they cost.
    Migration {
        name: "intelligence",
        sql: "
        CREATE TABLE catalog_models (
            id INTEGER PRIMARY KEY,
            provider_identifier TEXT NOT NULL,
            provider_name TEXT NOT NULL,
            model_identifier TEXT NOT NULL,
            model_name TEXT NOT NULL,
            context_size INTEGER,
            input_token_cost REAL,
            output_token_cost REAL,
            cache_read_token_cost REAL,
            cache_write_token_cost REAL,
            large_context_threshold INTEGER,
            large_context_input_token_cost REAL,
            large_context_output_token_cost REAL,
            large_context_cache_read_token_cost REAL,
            large_context_cache_write_token_cost REAL,
            UNIQUE (provider_identifier, model_identifier)
        );

        CREATE TABLE intelligence_endpoints (
            id INTEGER PRIMARY KEY,
            name TEXT COLLATE NOCASE UNIQUE,
            provider_identifier TEXT,
            dialect TEXT,
            base_url TEXT,
            api_key TEXT,
            client_id INTEGER UNIQUE REFERENCES clients(id) ON DELETE CASCADE,
            CHECK ((name IS NOT NULL AND provider_identifier IS NOT NULL
                    AND dialect IS NOT NULL AND base_url IS NOT NULL AND client_id IS NULL)
                OR (name IS NULL AND provider_identifier IS NULL AND dialect IS NULL
                    AND base_url IS NULL AND api_key IS NULL AND client_id IS NOT NULL))
        );

        CREATE TRIGGER intelligence_endpoints_live_client
        BEFORE INSERT ON intelligence_endpoints
        WHEN new.client_id IS NOT NULL
        AND (SELECT deleted FROM clients WHERE id = new.client_id)
        BEGIN
            SELECT RAISE(ABORT, 'A deleted client owns no endpoints.');
        END;

        CREATE TABLE endpoint_models (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id INTEGER NOT NULL
                REFERENCES intelligence_endpoints(id) ON DELETE CASCADE,
            model_identifier TEXT NOT NULL,
            model_name TEXT,
            catalog_model_id INTEGER REFERENCES catalog_models(id) ON DELETE SET NULL,
            context_size INTEGER,
            input_token_cost REAL,
            output_token_cost REAL,
            cache_read_token_cost REAL,
            cache_write_token_cost REAL,
            large_context_threshold INTEGER,
            large_context_input_token_cost REAL,
            large_context_output_token_cost REAL,
            large_context_cache_read_token_cost REAL,
            large_context_cache_write_token_cost REAL,
            UNIQUE (endpoint_id, model_identifier)
        );

        CREATE INDEX endpoint_models_catalog_model ON endpoint_models(catalog_model_id);

        CREATE TABLE intelligence_defaults (
            id INTEGER PRIMARY KEY,
            tier TEXT NOT NULL UNIQUE,
            endpoint_model_id INTEGER NOT NULL
                REFERENCES endpoint_models(id) ON DELETE CASCADE
        );

        CREATE INDEX intelligence_defaults_endpoint_model
            ON intelligence_defaults(endpoint_model_id);

        CREATE TABLE intelligence_threads (
            id INTEGER PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            search_eligible BOOLEAN NOT NULL DEFAULT FALSE,
            system_prompt TEXT NOT NULL,
            remote_id TEXT,
            remote_cursor INTEGER,
            agent_thread BOOLEAN NOT NULL DEFAULT FALSE
        );

        CREATE TABLE intelligence_thread_messages (
            id INTEGER PRIMARY KEY,
            thread_id INTEGER NOT NULL REFERENCES intelligence_threads(id) ON DELETE CASCADE,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            description TEXT NOT NULL,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL
        );

        CREATE INDEX intelligence_thread_messages_thread ON intelligence_thread_messages(thread_id, id);

        CREATE TABLE intelligence_cost_events (
            id INTEGER PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            provider_identifier TEXT NOT NULL,
            model_identifier TEXT NOT NULL,
            input_cost REAL NOT NULL,
            output_cost REAL NOT NULL,
            cache_read_cost REAL NOT NULL,
            cache_write_cost REAL NOT NULL,
            thread_id INTEGER REFERENCES intelligence_threads(id) ON DELETE SET NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX intelligence_cost_events_created_at ON intelligence_cost_events(created_at);

        CREATE INDEX intelligence_cost_events_thread_id ON intelligence_cost_events(thread_id);",
    },
    // Agents, their runs, messages, attachments and cues.
    Migration {
        name: "agents",
        sql: "
        CREATE TABLE intelligence_agents (
            id INTEGER PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            agent_name TEXT NOT NULL,
            agent_color TEXT NOT NULL,
            foreground_status TEXT NOT NULL DEFAULT '',
            system_defined_status TEXT NOT NULL DEFAULT 'idle',
            agent_emoji TEXT,
            agent_unread BOOLEAN NOT NULL DEFAULT FALSE,
            sort_order INTEGER NOT NULL DEFAULT 0,
            background_status TEXT
        );

        CREATE TRIGGER intelligence_agents_storage_ad AFTER DELETE ON intelligence_agents
        BEGIN
            DELETE FROM storage_entries WHERE sandbox = 'agent:' || old.id;
        END;

        CREATE TABLE intelligence_agent_runs (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            thread_id INTEGER REFERENCES intelligence_threads(id) ON DELETE SET NULL,
            hook TEXT NOT NULL,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            ended_at INTEGER,
            outcome TEXT NOT NULL DEFAULT 'running',
            error TEXT,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cost REAL,
            cue_name TEXT,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX intelligence_agent_runs_agent ON intelligence_agent_runs(agent_id, started_at);

        CREATE TABLE intelligence_agent_messages (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            run_id INTEGER REFERENCES intelligence_agent_runs(id) ON DELETE SET NULL,
            run_description TEXT,
            sent_at INTEGER NOT NULL DEFAULT (unixepoch()),
            sender TEXT NOT NULL,
            pinned BOOLEAN NOT NULL DEFAULT FALSE,
            message_content TEXT NOT NULL,
            reaction TEXT,
            reacted_at INTEGER,
            attachment_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX intelligence_agent_messages_agent ON intelligence_agent_messages(agent_id, id);

        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind') NOT IN ('status', 'codeExecution', 'documentation')
        BEGIN
            UPDATE intelligence_agents SET agent_unread = (new.sender <> 'user') WHERE id = new.agent_id;
        END;

        CREATE TABLE intelligence_agent_attachments (
            id INTEGER PRIMARY KEY,
            message_id INTEGER NOT NULL
                REFERENCES intelligence_agent_messages(id) ON DELETE CASCADE,
            media_type TEXT NOT NULL,
            data TEXT NOT NULL
        );

        CREATE INDEX intelligence_agent_attachments_message
            ON intelligence_agent_attachments(message_id);

        CREATE TABLE intelligence_agent_cues (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            hook TEXT NOT NULL,
            script TEXT NOT NULL,
            instructions TEXT NOT NULL,
            effort TEXT NOT NULL,
            UNIQUE (agent_id, name)
        );

        CREATE INDEX intelligence_agent_cues_hook ON intelligence_agent_cues(hook);",
    },

    // A connector's definition, shared by the extension or clients serving
    // it; accounts that outlive their host as orphans; imported assets
    // become resources, whose kind is stored as the connector's short key
    // and which carry metadata; and no mirror of what a client offers.
    Migration {
        name: "connector_definitions",
        sql: "
        CREATE TABLE connectors (
            id INTEGER PRIMARY KEY,
            identifier TEXT NOT NULL UNIQUE COLLATE NOCASE,
            version INTEGER NOT NULL,
            title TEXT NOT NULL,
            icon TEXT,
            unit TEXT NOT NULL,
            kinds TEXT NOT NULL DEFAULT '[]',
            metadata_keys TEXT NOT NULL DEFAULT '[]',
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );

        INSERT OR IGNORE INTO connectors (identifier, version, title, icon, unit, kinds)
            SELECT coalesce(a.extension_identifier, a.offer), 0, a.service_title, e.icon,
                a.account_unit, a.asset_kinds
            FROM connector_accounts a LEFT JOIN extensions e ON e.id = a.extension_id
            ORDER BY a.id DESC;

        DROP TRIGGER connector_accounts_live_client;

        ALTER TABLE connector_accounts RENAME TO connector_accounts_old;

        CREATE TABLE connector_accounts (
            id INTEGER PRIMARY KEY,
            connector_id INTEGER NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
            extension_id INTEGER REFERENCES extensions(id) ON DELETE SET NULL,
            identifier TEXT,
            authorization TEXT,
            refresh TEXT,
            expires_at INTEGER,
            client_id INTEGER REFERENCES clients(id),
            description TEXT,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK (extension_id IS NULL OR client_id IS NULL),
            CHECK ((extension_id IS NULL) = (identifier IS NULL)
                AND (extension_id IS NULL) = (authorization IS NULL)
                AND (extension_id IS NOT NULL OR (refresh IS NULL AND expires_at IS NULL))),
            CHECK (client_id IS NOT NULL OR description IS NOT NULL),
            UNIQUE (extension_id, identifier),
            UNIQUE (client_id, connector_id)
        );

        INSERT INTO connector_accounts (id, connector_id, extension_id, identifier,
                authorization, refresh, expires_at, client_id, description, created_at)
            SELECT a.id, c.id, a.extension_id, a.identifier, a.authorization, a.refresh,
                a.expires_at, a.client_id, a.description, a.created_at
            FROM connector_accounts_old a
            JOIN connectors c ON c.identifier = coalesce(a.extension_identifier, a.offer);

        CREATE TRIGGER connector_accounts_live_client
        BEFORE INSERT ON connector_accounts
        WHEN new.client_id IS NOT NULL
        AND (SELECT deleted FROM clients WHERE id = new.client_id)
        BEGIN
            SELECT RAISE(ABORT, 'A deleted client owns no accounts.');
        END;

        CREATE TRIGGER extensions_orphan_accounts BEFORE DELETE ON extensions
        BEGIN
            UPDATE connector_accounts
            SET extension_id = NULL, identifier = NULL, authorization = NULL,
                refresh = NULL, expires_at = NULL
            WHERE extension_id = old.id;
        END;

        DROP TRIGGER clients_deleted_own_nothing;

        CREATE TRIGGER clients_deleted_own_nothing
        AFTER UPDATE OF deleted ON clients
        WHEN new.deleted
        BEGIN
            DELETE FROM intelligence_endpoints WHERE client_id = new.id;
            UPDATE connector_accounts SET client_id = NULL, description = new.client_name
            WHERE client_id = new.id;
        END;

        DROP TABLE edge_resources;

        DROP TRIGGER assets_search_ai;
        DROP TRIGGER assets_search_ad;
        DROP TRIGGER assets_search_au;
        DELETE FROM search_fts WHERE rowid IN
            (SELECT id FROM search_entries WHERE entity_kind = 'asset');
        DELETE FROM search_entries WHERE entity_kind = 'asset';

        CREATE TABLE resources (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            account_id INTEGER NOT NULL REFERENCES connector_accounts(id) ON DELETE CASCADE,
            identifier TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            content_kind TEXT NOT NULL,
            contents TEXT NOT NULL,
            last_updated INTEGER NOT NULL,
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (workspace_id, account_id, identifier)
        );

        CREATE INDEX resources_workspace_kind ON resources(workspace_id, kind);

        CREATE INDEX resources_account ON resources(account_id);

        CREATE TRIGGER resources_same_connector
        BEFORE UPDATE OF account_id ON resources
        WHEN (SELECT connector_id FROM connector_accounts WHERE id = old.account_id)
            IS NOT (SELECT connector_id FROM connector_accounts WHERE id = new.account_id)
        BEGIN
            SELECT RAISE(ABORT, 'A resource moves only to an account of its connector.');
        END;

        CREATE TRIGGER resources_search_ai AFTER INSERT ON resources
        BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('resource', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title,
                    iif(new.content_kind = 'json', '', new.contents));
        END;

        CREATE TRIGGER resources_search_ad AFTER DELETE ON resources
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id;
        END;

        CREATE TRIGGER resources_search_au
        AFTER UPDATE OF title, contents, content_kind, workspace_id ON resources
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('resource', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title,
                    iif(new.content_kind = 'json', '', new.contents));
        END;

        CREATE TRIGGER resources_metadata_ad AFTER DELETE ON resources
        BEGIN
            DELETE FROM metadata WHERE entity_kind = 'resources' AND entity_id = old.id;
        END;

        INSERT INTO resources (id, workspace_id, account_id, identifier, kind, title,
                content_kind, contents, last_updated, modified)
            SELECT r.id, r.workspace_id, r.connector_account_id, r.identifier,
                substr(r.kind, length(coalesce(a.extension_identifier, 'client.' || a.offer)) + 2),
                r.title, r.content_kind, r.contents, r.last_updated, r.modified
            FROM assets r JOIN connector_accounts_old a ON a.id = r.connector_account_id;

        DROP TABLE assets;

        DROP TABLE connector_accounts_old;",
    },
    // Extension versions become whole numbers, compared to find updates and
    // to version a connector definition. A stored version that isn't one
    // becomes 0, so any numbered release counts as an update.
    Migration {
        name: "extension_versions",
        sql: "
        ALTER TABLE extensions ADD COLUMN version_number INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE extensions ADD COLUMN announced_number INTEGER;

        UPDATE extensions SET
            version_number = iif(version <> '' AND version NOT GLOB '*[^0-9]*',
                CAST(version AS INTEGER), 0),
            announced_number = iif(announced_version IS NULL, NULL,
                iif(announced_version <> '' AND announced_version NOT GLOB '*[^0-9]*',
                    CAST(announced_version AS INTEGER), 0));

        ALTER TABLE extensions DROP COLUMN version;
        ALTER TABLE extensions DROP COLUMN announced_version;
        ALTER TABLE extensions RENAME COLUMN version_number TO version;
        ALTER TABLE extensions RENAME COLUMN announced_number TO announced_version;",
    },
    // A connector's kinds become rows its resources point at, each with a
    // name for several; a definition that drops a kind drops its
    // resources. A kind is shown in a workspace only once it is turned on
    // there; each kind a workspace already holds resources of starts on. A
    // stored kind takes its title as its plural until its connector's next
    // version, and a resource's kind its connector never declared becomes
    // one, named by its key.
    Migration {
        name: "resource_kinds",
        sql: "
        CREATE TABLE resource_kinds (
            id INTEGER PRIMARY KEY,
            connector_id INTEGER NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
            identifier TEXT NOT NULL,
            title TEXT NOT NULL,
            plural TEXT NOT NULL,
            contents TEXT NOT NULL,
            UNIQUE (connector_id, identifier)
        );

        INSERT OR IGNORE INTO resource_kinds (connector_id, identifier, title, plural, contents)
            SELECT c.id, k.value ->> 'identifier', k.value ->> 'title', k.value ->> 'title',
                k.value ->> 'contents'
            FROM connectors c, json_each(c.kinds) k
            ORDER BY c.id, k.key;

        INSERT OR IGNORE INTO resource_kinds (connector_id, identifier, title, plural, contents)
            SELECT DISTINCT a.connector_id, r.kind, r.kind, r.kind, ''
            FROM resources r JOIN connector_accounts a ON a.id = r.account_id;

        ALTER TABLE connectors DROP COLUMN kinds;

        DROP TRIGGER resources_same_connector;
        DROP TRIGGER resources_search_ai;
        DROP TRIGGER resources_search_ad;
        DROP TRIGGER resources_search_au;
        DROP TRIGGER resources_metadata_ad;
        DROP INDEX resources_workspace_kind;
        DROP INDEX resources_account;

        CREATE TABLE resources_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            account_id INTEGER NOT NULL REFERENCES connector_accounts(id) ON DELETE CASCADE,
            kind_id INTEGER NOT NULL REFERENCES resource_kinds(id) ON DELETE CASCADE,
            identifier TEXT NOT NULL,
            title TEXT NOT NULL,
            content_kind TEXT NOT NULL,
            contents TEXT NOT NULL,
            last_updated INTEGER NOT NULL,
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (workspace_id, account_id, identifier)
        );

        INSERT INTO resources_new (id, workspace_id, account_id, kind_id, identifier, title,
                content_kind, contents, last_updated, modified)
            SELECT r.id, r.workspace_id, r.account_id, k.id, r.identifier, r.title,
                r.content_kind, r.contents, r.last_updated, r.modified
            FROM resources r
            JOIN connector_accounts a ON a.id = r.account_id
            JOIN resource_kinds k ON k.connector_id = a.connector_id AND k.identifier = r.kind;

        DROP TABLE resources;
        ALTER TABLE resources_new RENAME TO resources;

        CREATE INDEX resources_workspace_kind ON resources(workspace_id, kind_id);

        CREATE INDEX resources_account ON resources(account_id);

        CREATE INDEX resources_kind ON resources(kind_id);

        CREATE TRIGGER resources_same_connector
        BEFORE UPDATE OF account_id ON resources
        WHEN (SELECT connector_id FROM connector_accounts WHERE id = old.account_id)
            IS NOT (SELECT connector_id FROM connector_accounts WHERE id = new.account_id)
        BEGIN
            SELECT RAISE(ABORT, 'A resource moves only to an account of its connector.');
        END;

        CREATE TRIGGER resources_search_ai AFTER INSERT ON resources
        BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('resource', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title,
                    iif(new.content_kind = 'json', '', new.contents));
        END;

        CREATE TRIGGER resources_search_ad AFTER DELETE ON resources
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id;
        END;

        CREATE TRIGGER resources_search_au
        AFTER UPDATE OF title, contents, content_kind, workspace_id ON resources
        BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'resource' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('resource', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title,
                    iif(new.content_kind = 'json', '', new.contents));
        END;

        CREATE TRIGGER resources_metadata_ad AFTER DELETE ON resources
        BEGIN
            DELETE FROM metadata WHERE entity_kind = 'resources' AND entity_id = old.id;
        END;

        CREATE TABLE workspace_resource_kinds (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            kind_id INTEGER NOT NULL REFERENCES resource_kinds(id) ON DELETE CASCADE,
            UNIQUE (workspace_id, kind_id)
        );

        CREATE INDEX workspace_resource_kinds_kind ON workspace_resource_kinds(kind_id);

        INSERT INTO workspace_resource_kinds (workspace_id, kind_id)
            SELECT DISTINCT workspace_id, kind_id FROM resources ORDER BY workspace_id, kind_id;",
    },
    // The kinds a workspace shows take an order of their own, starting in
    // their connectors' order.
    Migration {
        name: "workspace_resource_kind_order",
        sql: "
        ALTER TABLE workspace_resource_kinds ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

        UPDATE workspace_resource_kinds SET sort_order = ordered.position
        FROM (
            SELECT w.id, ROW_NUMBER() OVER (
                PARTITION BY w.workspace_id ORDER BY k.connector_id, k.id
            ) AS position
            FROM workspace_resource_kinds w JOIN resource_kinds k ON k.id = w.kind_id
        ) AS ordered
        WHERE ordered.id = workspace_resource_kinds.id;",
    },
    // Tags: names a workspace puts on its tasks, or on the articles saved to
    // it, each kind with its own. A tag stays on an item only while the item
    // is in the tag's workspace.
    Migration {
        name: "tags",
        sql: "
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            entity TEXT NOT NULL,
            name TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (workspace_id, entity, name COLLATE NOCASE)
        );

        CREATE TABLE tagged (
            id INTEGER PRIMARY KEY,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            entity_id INTEGER NOT NULL,
            UNIQUE (tag_id, entity_id)
        );

        CREATE INDEX tagged_entity ON tagged(entity_id);

        CREATE TRIGGER tagged_in_workspace BEFORE INSERT ON tagged
        WHEN NOT EXISTS (
            SELECT 1 FROM tags WHERE tags.id = new.tag_id AND (
                (tags.entity = 'task' AND EXISTS (
                    SELECT 1 FROM tasks
                    WHERE tasks.id = new.entity_id AND tasks.workspace_id = tags.workspace_id))
                OR (tags.entity = 'article' AND EXISTS (
                    SELECT 1 FROM workspace_articles
                    WHERE workspace_articles.article_id = new.entity_id
                        AND workspace_articles.workspace_id = tags.workspace_id))
            )
        )
        BEGIN
            SELECT RAISE(ABORT, 'A tag goes only on an item in its workspace.');
        END;

        CREATE TRIGGER tasks_tagged_ad AFTER DELETE ON tasks
        BEGIN
            DELETE FROM tagged WHERE entity_id = old.id
                AND tag_id IN (SELECT id FROM tags WHERE entity = 'task');
        END;

        CREATE TRIGGER workspace_articles_tagged_ad AFTER DELETE ON workspace_articles
        BEGIN
            DELETE FROM tagged WHERE entity_id = old.article_id
                AND tag_id IN (
                    SELECT id FROM tags
                    WHERE entity = 'article' AND workspace_id = old.workspace_id
                );
        END;",
    },
    // How an agent speaks: its personality and how much it says.
    Migration {
        name: "agent_voice",
        sql: "
        ALTER TABLE intelligence_agents
            ADD COLUMN agent_personality TEXT NOT NULL DEFAULT 'balanced';
        ALTER TABLE intelligence_agents
            ADD COLUMN agent_verbosity TEXT NOT NULL DEFAULT 'brief';",
    },
    // Whether an article's publication is a moment or only a day. Dates
    // read before were stored as midnight UTC, which only a day is.
    Migration {
        name: "article_published_precision",
        sql: "
        ALTER TABLE articles
            ADD COLUMN published_precision TEXT NOT NULL DEFAULT 'time';
        UPDATE articles SET published_precision = 'day' WHERE published_at % 86400 = 0;",
    },
    // What reads a feed's item pages. Every feed starts on Cohesion's own
    // parser.
    Migration {
        name: "feed_parser",
        sql: "
        ALTER TABLE feeds ADD COLUMN parser TEXT NOT NULL DEFAULT 'coherence';",
    },
    // Tags take an order of their own, starting by name.
    Migration {
        name: "tag_order",
        sql: "
        ALTER TABLE tags ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

        UPDATE tags SET sort_order = ordered.position
        FROM (
            SELECT id, ROW_NUMBER() OVER (
                PARTITION BY workspace_id, entity ORDER BY name COLLATE NOCASE, id
            ) AS position
            FROM tags
        ) AS ordered
        WHERE ordered.id = tags.id;",
    },
    // Agents can be pinned; the rest go by their latest message.
    Migration {
        name: "agent_pinned",
        sql: "
        ALTER TABLE intelligence_agents ADD COLUMN pinned BOOLEAN NOT NULL DEFAULT FALSE;",
    },
    // An agent records when its conversation last had a message, counting
    // the same kinds the unread trigger does, which now records it too.
    Migration {
        name: "agent_last_message_at",
        sql: "
        ALTER TABLE intelligence_agents ADD COLUMN last_message_at INTEGER;

        UPDATE intelligence_agents SET last_message_at = (
            SELECT MAX(sent_at) FROM intelligence_agent_messages
            WHERE agent_id = intelligence_agents.id
                AND json_extract(message_content, '$.kind') NOT IN ('status', 'codeExecution', 'documentation')
        );

        DROP TRIGGER intelligence_agent_messages_unread;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind') NOT IN ('status', 'codeExecution', 'documentation')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // A cost event keeps its total, which a provider may report without the
    // per-kind costs.
    Migration {
        name: "cost_event_total",
        sql: "
        DROP INDEX intelligence_cost_events_created_at;
        DROP INDEX intelligence_cost_events_thread_id;

        CREATE TABLE intelligence_cost_events_new (
            id INTEGER PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            thread_id INTEGER REFERENCES intelligence_threads(id) ON DELETE SET NULL,
            provider_identifier TEXT NOT NULL,
            model_identifier TEXT NOT NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost REAL NOT NULL,
            input_cost REAL,
            output_cost REAL,
            cache_read_cost REAL,
            cache_write_cost REAL
        );

        INSERT INTO intelligence_cost_events_new (id, created_at, thread_id, provider_identifier,
                model_identifier, input_tokens, output_tokens, cache_read_tokens,
                cache_write_tokens, total_cost, input_cost, output_cost, cache_read_cost,
                cache_write_cost)
            SELECT id, created_at, thread_id, provider_identifier, model_identifier,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                input_cost + output_cost + cache_read_cost + cache_write_cost,
                input_cost, output_cost, cache_read_cost, cache_write_cost
            FROM intelligence_cost_events;

        DROP TABLE intelligence_cost_events;
        ALTER TABLE intelligence_cost_events_new RENAME TO intelligence_cost_events;

        CREATE INDEX intelligence_cost_events_created_at ON intelligence_cost_events(created_at);

        CREATE INDEX intelligence_cost_events_thread_id ON intelligence_cost_events(thread_id);",
    },
    // Agents keep memories, personal or global, searched by word; each
    // pinned text message becomes a personal memory of its agent, and pins
    // go. A `memory` message, like a status, leaves the agent read.
    Migration {
        name: "memories",
        sql: "
        CREATE TABLE memories (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            memory TEXT NOT NULL
        );

        CREATE INDEX memories_agent ON memories(agent_id);

        CREATE VIRTUAL TABLE memories_fts USING fts5(memory,
            tokenize='porter unicode61 remove_diacritics 2');

        CREATE TRIGGER memories_fts_ai AFTER INSERT ON memories
        BEGIN
            INSERT INTO memories_fts (rowid, memory) VALUES (new.id, new.memory);
        END;

        CREATE TRIGGER memories_fts_ad AFTER DELETE ON memories
        BEGIN
            DELETE FROM memories_fts WHERE rowid = old.id;
        END;

        INSERT INTO memories (agent_id, created_at, memory)
            SELECT agent_id, sent_at, json_extract(message_content, '$.text')
            FROM intelligence_agent_messages
            WHERE pinned AND json_extract(message_content, '$.kind') = 'text'
                AND json_extract(message_content, '$.text') <> ''
            ORDER BY id;

        ALTER TABLE intelligence_agent_messages DROP COLUMN pinned;

        DROP TRIGGER intelligence_agent_messages_unread;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind')
            NOT IN ('status', 'codeExecution', 'documentation', 'memory')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // An agent's three statuses become one `activity`, set by the daemon
    // only while work runs; the status lines agents posted go with them.
    Migration {
        name: "agent_activity",
        sql: "
        ALTER TABLE intelligence_agents DROP COLUMN foreground_status;
        ALTER TABLE intelligence_agents DROP COLUMN background_status;
        ALTER TABLE intelligence_agents DROP COLUMN system_defined_status;
        ALTER TABLE intelligence_agents ADD COLUMN activity TEXT;

        DELETE FROM intelligence_agent_messages
        WHERE json_extract(message_content, '$.kind') = 'status';

        DROP TRIGGER intelligence_agent_messages_unread;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind')
            NOT IN ('codeExecution', 'documentation', 'memory')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // An agent's role, what the user wants from it, and its directive, the
    // rules it follows; both the user's to write.
    Migration {
        name: "agent_role_directive",
        sql: "
        ALTER TABLE intelligence_agents ADD COLUMN agent_role TEXT NOT NULL DEFAULT '';
        ALTER TABLE intelligence_agents ADD COLUMN agent_directive TEXT NOT NULL DEFAULT '';",
    },
    // The rights an agent's background runs hold beyond the base every run
    // has; a row is a grant held.
    Migration {
        name: "intelligence_agent_grants",
        sql: "
        CREATE TABLE intelligence_agent_grants (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            UNIQUE (agent_id, kind)
        );",
    },
    // Long-running work the user follows from one place, each row one
    // activity of a kind; and the runs an agent detaches to work on their
    // own, which an activity shows.
    Migration {
        name: "activities",
        sql: "
        CREATE TABLE activities (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            agent_id INTEGER REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            status TEXT NOT NULL DEFAULT 'running',
            progress TEXT,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            ended_at INTEGER
        );

        CREATE INDEX activities_started ON activities(ended_at, started_at);

        -- Triggers rather than CHECKs, so a later kind or status replaces
        -- them without rebuilding the table: a row is exactly one kind, and
        -- has ended exactly when its status is an outcome.
        CREATE TRIGGER activities_valid_insert BEFORE INSERT ON activities
        WHEN new.agent_id IS NULL
            OR new.status NOT IN ('running', 'waiting', 'paused', 'succeeded', 'failed', 'cancelled')
            OR (new.ended_at IS NULL) <> (new.status IN ('running', 'waiting', 'paused'))
        BEGIN
            SELECT RAISE(ABORT, 'An activity is one kind, and ended exactly when it has an outcome.');
        END;

        CREATE TRIGGER activities_valid_update BEFORE UPDATE ON activities
        WHEN new.agent_id IS NULL
            OR new.status NOT IN ('running', 'waiting', 'paused', 'succeeded', 'failed', 'cancelled')
            OR (new.ended_at IS NULL) <> (new.status IN ('running', 'waiting', 'paused'))
        BEGIN
            SELECT RAISE(ABORT, 'An activity is one kind, and ended exactly when it has an outcome.');
        END;

        CREATE TABLE intelligence_agent_detached_runs (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            activity_id INTEGER NOT NULL UNIQUE REFERENCES activities(id) ON DELETE CASCADE,
            thread_id INTEGER REFERENCES intelligence_threads(id) ON DELETE SET NULL,
            instructions TEXT NOT NULL,
            effort TEXT NOT NULL,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            ended_at INTEGER,
            outcome TEXT NOT NULL DEFAULT 'running',
            error TEXT,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0,
            cost REAL
        );

        CREATE INDEX intelligence_agent_detached_runs_agent
            ON intelligence_agent_detached_runs(agent_id, started_at);",
    },
    // Actors, the agents a client offers on its device, the tasks Cohesion
    // hands them and what is said in each; a stretch of a task's work is an
    // activity of its own kind.
    Migration {
        name: "actors",
        sql: "
        CREATE TABLE actors (
            id INTEGER PRIMARY KEY,
            deleted INTEGER NOT NULL DEFAULT FALSE,
            client_id INTEGER NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
            identifier TEXT NOT NULL,
            product TEXT NOT NULL,
            scope TEXT NOT NULL,
            modes TEXT NOT NULL,
            models TEXT NOT NULL,
            UNIQUE (client_id, identifier)
        );

        CREATE TABLE actor_tasks (
            id INTEGER PRIMARY KEY,
            actor_id INTEGER NOT NULL REFERENCES actors(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            mode TEXT,
            model TEXT,
            effort TEXT,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            last_activity_at INTEGER NOT NULL DEFAULT (unixepoch()),
            outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'cancelled')),
            ended_at INTEGER,
            CHECK ((outcome IS NULL) = (ended_at IS NULL))
        );

        CREATE INDEX actor_tasks_recent ON actor_tasks(last_activity_at);

        CREATE TABLE actor_task_messages (
            id INTEGER PRIMARY KEY,
            task_id INTEGER NOT NULL REFERENCES actor_tasks(id) ON DELETE CASCADE,
            sent_at INTEGER NOT NULL DEFAULT (unixepoch()),
            content TEXT NOT NULL
        );

        CREATE INDEX actor_task_messages_task ON actor_task_messages(task_id, id);

        -- A request is answered at most once.
        CREATE UNIQUE INDEX actor_task_messages_one_response
            ON actor_task_messages(json_extract(content, '$.request_id'))
            WHERE json_extract(content, '$.request_id') IS NOT NULL;

        CREATE TRIGGER actor_task_messages_activity AFTER INSERT ON actor_task_messages
        BEGIN
            UPDATE actor_tasks SET last_activity_at = new.sent_at WHERE id = new.task_id;
        END;

        DROP TRIGGER clients_deleted_own_nothing;
        CREATE TRIGGER clients_deleted_own_nothing
        AFTER UPDATE OF deleted ON clients
        WHEN new.deleted
        BEGIN
            DELETE FROM intelligence_endpoints WHERE client_id = new.id;
            UPDATE connector_accounts SET client_id = NULL, description = new.client_name
            WHERE client_id = new.id;
            UPDATE actors SET deleted = TRUE WHERE client_id = new.id;
        END;

        ALTER TABLE activities ADD COLUMN
            actor_task_id INTEGER REFERENCES actor_tasks(id) ON DELETE CASCADE;

        -- One stretch of a task's work goes on at a time.
        CREATE UNIQUE INDEX activities_one_open_actor_task ON activities(actor_task_id)
            WHERE actor_task_id IS NOT NULL AND ended_at IS NULL;

        DROP TRIGGER activities_valid_insert;
        CREATE TRIGGER activities_valid_insert BEFORE INSERT ON activities
        WHEN (new.agent_id IS NULL) = (new.actor_task_id IS NULL)
            OR new.status NOT IN ('running', 'waiting', 'paused', 'succeeded', 'failed', 'cancelled')
            OR (new.ended_at IS NULL) <> (new.status IN ('running', 'waiting', 'paused'))
        BEGIN
            SELECT RAISE(ABORT, 'An activity is one kind, and ended exactly when it has an outcome.');
        END;

        DROP TRIGGER activities_valid_update;
        CREATE TRIGGER activities_valid_update BEFORE UPDATE ON activities
        WHEN (new.agent_id IS NULL) = (new.actor_task_id IS NULL)
            OR new.status NOT IN ('running', 'waiting', 'paused', 'succeeded', 'failed', 'cancelled')
            OR (new.ended_at IS NULL) <> (new.status IN ('running', 'waiting', 'paused'))
        BEGIN
            SELECT RAISE(ABORT, 'An activity is one kind, and ended exactly when it has an outcome.');
        END;",
    },
    // An actor task keeps its own status, as its actor reports it: working,
    // needing attention, idle or failed. None ends for good, so `outcome`
    // and `ended_at` go: a failed task is failed, any other idle, since a
    // stopped daemon runs none. Dropping them takes rebuilding the table,
    // whose CHECK names them; what was said in each task and its
    // activities are kept aside, since dropping the old table deletes them.
    Migration {
        name: "actor_task_status",
        sql: "
        CREATE TEMP TABLE actor_task_messages_kept AS SELECT * FROM actor_task_messages;
        CREATE TEMP TABLE actor_task_activities_kept AS
            SELECT * FROM activities WHERE actor_task_id IS NOT NULL;
        DROP TRIGGER actor_task_messages_activity;

        CREATE TABLE actor_tasks_new (
            id INTEGER PRIMARY KEY,
            actor_id INTEGER NOT NULL REFERENCES actors(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            mode TEXT,
            model TEXT,
            effort TEXT,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            last_activity_at INTEGER NOT NULL DEFAULT (unixepoch()),
            status TEXT NOT NULL DEFAULT 'idle'
        );

        INSERT INTO actor_tasks_new (id, actor_id, title, mode, model, effort, started_at,
                last_activity_at, status)
            SELECT id, actor_id, title, mode, model, effort, started_at, last_activity_at,
                iif(outcome = 'failed', 'failed', 'idle')
            FROM actor_tasks;

        DROP TABLE actor_tasks;
        ALTER TABLE actor_tasks_new RENAME TO actor_tasks;

        CREATE INDEX actor_tasks_recent ON actor_tasks(last_activity_at);

        INSERT INTO actor_task_messages SELECT * FROM actor_task_messages_kept ORDER BY id;
        INSERT INTO activities SELECT * FROM actor_task_activities_kept ORDER BY id;
        DROP TABLE actor_task_messages_kept;
        DROP TABLE actor_task_activities_kept;

        CREATE TRIGGER actor_task_messages_activity AFTER INSERT ON actor_task_messages
        BEGIN
            UPDATE actor_tasks SET last_activity_at = new.sent_at WHERE id = new.task_id;
        END;

        -- Triggers rather than a CHECK, as for activities, so a later status
        -- replaces them without rebuilding the table.
        CREATE TRIGGER actor_tasks_valid_insert BEFORE INSERT ON actor_tasks
        WHEN new.status NOT IN ('working', 'needsAttention', 'idle', 'failed')
        BEGIN
            SELECT RAISE(ABORT, 'An actor task is working, needs attention, idle or failed.');
        END;

        CREATE TRIGGER actor_tasks_valid_update BEFORE UPDATE OF status ON actor_tasks
        WHEN new.status NOT IN ('working', 'needsAttention', 'idle', 'failed')
        BEGIN
            SELECT RAISE(ABORT, 'An actor task is working, needs attention, idle or failed.');
        END;",
    },
    // An actor task is unread from when its actor finishes a stretch of
    // work, idle or failed, until the user reads it. The stretch's activity
    // ending succeeded or failed is what says the actor finished it: a stop,
    // or its client or the daemon going away, cancels it instead.
    Migration {
        name: "actor_task_unread",
        sql: "
        ALTER TABLE actor_tasks ADD COLUMN unread BOOLEAN NOT NULL DEFAULT FALSE;

        CREATE TRIGGER activities_actor_task_unread AFTER UPDATE OF status ON activities
        WHEN new.actor_task_id IS NOT NULL AND new.status IN ('succeeded', 'failed')
        BEGIN
            UPDATE actor_tasks SET unread = TRUE WHERE id = new.actor_task_id;
        END;",
    },
    // Images the user attaches to a message in an actor task, kept beside
    // the transcript as an agent's are, so reading it doesn't carry them.
    Migration {
        name: "actor_task_attachments",
        sql: "
        ALTER TABLE actor_task_messages ADD COLUMN
            attachment_count INTEGER NOT NULL DEFAULT 0;

        CREATE TABLE actor_task_attachments (
            id INTEGER PRIMARY KEY,
            message_id INTEGER NOT NULL
                REFERENCES actor_task_messages(id) ON DELETE CASCADE,
            media_type TEXT NOT NULL,
            data TEXT NOT NULL
        );

        CREATE INDEX actor_task_attachments_message
            ON actor_task_attachments(message_id);",
    },
    // A cue can wake its agent's conversation once rather than a background
    // run each time; such a cue has no effort, the conversation's model
    // being the `agent` tier's.
    Migration {
        name: "agent_cue_wake",
        sql: "
        CREATE TABLE intelligence_agent_cues_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            hook TEXT NOT NULL,
            script TEXT NOT NULL,
            instructions TEXT NOT NULL,
            effort TEXT,
            UNIQUE (agent_id, name)
        );

        INSERT INTO intelligence_agent_cues_new
            (id, agent_id, name, hook, script, instructions, effort)
            SELECT id, agent_id, name, hook, script, instructions, effort
            FROM intelligence_agent_cues;

        DROP TABLE intelligence_agent_cues;
        ALTER TABLE intelligence_agent_cues_new RENAME TO intelligence_agent_cues;

        CREATE INDEX intelligence_agent_cues_hook ON intelligence_agent_cues(hook);",
    },
    // A foreground cue's effort is `foreground` rather than NULL, so every
    // cue has one again.
    Migration {
        name: "agent_cue_foreground_effort",
        sql: "
        CREATE TABLE intelligence_agent_cues_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            hook TEXT NOT NULL,
            script TEXT NOT NULL,
            instructions TEXT NOT NULL,
            effort TEXT NOT NULL,
            UNIQUE (agent_id, name)
        );

        INSERT INTO intelligence_agent_cues_new
            (id, agent_id, name, hook, script, instructions, effort)
            SELECT id, agent_id, name, hook, script, instructions,
                COALESCE(effort, 'foreground')
            FROM intelligence_agent_cues;

        DROP TABLE intelligence_agent_cues;
        ALTER TABLE intelligence_agent_cues_new RENAME TO intelligence_agent_cues;

        CREATE INDEX intelligence_agent_cues_hook ON intelligence_agent_cues(hook);",
    },
    // Full-text indexes of actor tasks, beside the workspaces' since a task
    // belongs to none: one of their titles, one of the text the user and
    // the actor said in them (not tool uses, requests or responses), each
    // row keyed by its task's or message's id.
    Migration {
        name: "actor_task_search",
        sql: "
        CREATE VIRTUAL TABLE actor_tasks_fts USING fts5(title,
            tokenize='porter unicode61 remove_diacritics 2');

        CREATE VIRTUAL TABLE actor_task_messages_fts USING fts5(text,
            tokenize='porter unicode61 remove_diacritics 2');

        INSERT INTO actor_tasks_fts (rowid, title) SELECT id, title FROM actor_tasks;

        INSERT INTO actor_task_messages_fts (rowid, text)
            SELECT id, content ->> '$.text' FROM actor_task_messages
            WHERE content ->> '$.kind' = 'text';

        CREATE TRIGGER actor_tasks_search_ai AFTER INSERT ON actor_tasks
        BEGIN
            INSERT INTO actor_tasks_fts (rowid, title) VALUES (new.id, new.title);
        END;

        CREATE TRIGGER actor_tasks_search_ad AFTER DELETE ON actor_tasks
        BEGIN
            DELETE FROM actor_tasks_fts WHERE rowid = old.id;
        END;

        CREATE TRIGGER actor_tasks_search_au AFTER UPDATE OF title ON actor_tasks
        BEGIN
            DELETE FROM actor_tasks_fts WHERE rowid = old.id;
            INSERT INTO actor_tasks_fts (rowid, title) VALUES (new.id, new.title);
        END;

        CREATE TRIGGER actor_task_messages_search_ai AFTER INSERT ON actor_task_messages
        WHEN new.content ->> '$.kind' = 'text'
        BEGIN
            INSERT INTO actor_task_messages_fts (rowid, text)
                VALUES (new.id, new.content ->> '$.text');
        END;

        CREATE TRIGGER actor_task_messages_search_ad AFTER DELETE ON actor_task_messages
        BEGIN
            DELETE FROM actor_task_messages_fts WHERE rowid = old.id;
        END;

        CREATE TRIGGER actor_task_messages_search_au AFTER UPDATE OF content ON actor_task_messages
        BEGIN
            DELETE FROM actor_task_messages_fts WHERE rowid = old.id;
            INSERT INTO actor_task_messages_fts (rowid, text)
                SELECT new.id, new.content ->> '$.text' WHERE new.content ->> '$.kind' = 'text';
        END;",
    },
    // An endpoint can link to the page where a person sees what it has
    // used. Endpoints from before have none until the user gives one.
    Migration {
        name: "endpoint_usage_url",
        sql: "
        ALTER TABLE intelligence_endpoints ADD COLUMN usage_url TEXT
            CHECK (usage_url IS NULL OR client_id IS NULL);",
    },
    // Actor tasks are ordered by when their status last changed, not by
    // when anything was last said in them, so a task that keeps talking
    // stays put while its status holds. Each task keeps its last message's
    // time until its status next changes.
    Migration {
        name: "actor_task_status_changed_at",
        sql: "
        DROP TRIGGER actor_task_messages_activity;
        ALTER TABLE actor_tasks RENAME COLUMN last_activity_at TO status_changed_at;

        CREATE TRIGGER actor_tasks_status_changed AFTER UPDATE OF status ON actor_tasks
        WHEN new.status <> old.status
        BEGIN
            UPDATE actor_tasks SET status_changed_at = unixepoch() WHERE id = new.id;
        END;",
    },
    // When the user last read an actor task, so a client can keep one just
    // read in view for a while. Tasks read before have none.
    Migration {
        name: "actor_task_read_at",
        sql: "ALTER TABLE actor_tasks ADD COLUMN read_at INTEGER;",
    },
    // How full each actor task's context window is, as its actor last
    // reported, rather than one figure per actor that its tasks overwrite.
    // Tasks from before have none until their actor next says.
    Migration {
        name: "actor_task_context",
        sql: "
        ALTER TABLE actor_tasks ADD COLUMN context_used_tokens INTEGER
            CHECK (context_used_tokens >= 0);
        ALTER TABLE actor_tasks ADD COLUMN context_total_tokens INTEGER
            CHECK ((context_total_tokens IS NULL) = (context_used_tokens IS NULL)
                AND (context_total_tokens IS NULL OR context_total_tokens > 0));",
    },
    // An endpoint can sign in with an extension's account instead of an API
    // key, each request carrying the account's credential. The endpoint is
    // the account's: it goes when the account is removed, or orphaned by
    // uninstalling its extension, which leaves it no credential.
    Migration {
        name: "endpoint_account",
        sql: "
        ALTER TABLE intelligence_endpoints ADD COLUMN account_id INTEGER
            REFERENCES connector_accounts(id) ON DELETE CASCADE
            CHECK (account_id IS NULL OR (api_key IS NULL AND client_id IS NULL));

        CREATE INDEX intelligence_endpoints_account ON intelligence_endpoints(account_id);

        CREATE TRIGGER connector_accounts_orphaned_endpoints
        AFTER UPDATE OF extension_id ON connector_accounts
        WHEN new.extension_id IS NULL
        BEGIN
            DELETE FROM intelligence_endpoints WHERE account_id = new.id;
        END;",
    },
    // Saved task views: a workspace's ways of looking at its tasks, kept
    // under a name. A view lists one of the lists every workspace has or
    // one of its live folders, and goes with the folder; it narrows by the
    // workspace's task tags, each wanted or unwanted, and loses a tag that
    // is deleted.
    Migration {
        name: "saved_task_views",
        sql: "
        CREATE TABLE saved_task_views (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            list TEXT,
            task_folder_id INTEGER REFERENCES task_folders(id) ON DELETE CASCADE,
            sort TEXT NOT NULL,
            descending INTEGER NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            UNIQUE (workspace_id, name COLLATE NOCASE),
            CHECK ((list IS NULL) <> (task_folder_id IS NULL))
        );

        CREATE INDEX saved_task_views_folder ON saved_task_views(task_folder_id);

        CREATE TABLE saved_task_view_tags (
            id INTEGER PRIMARY KEY,
            view_id INTEGER NOT NULL REFERENCES saved_task_views(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            tag_match TEXT NOT NULL,
            UNIQUE (view_id, tag_id)
        );

        CREATE INDEX saved_task_view_tags_tag ON saved_task_view_tags(tag_id);

        CREATE TRIGGER saved_task_views_folder_in_workspace
        BEFORE INSERT ON saved_task_views
        WHEN new.task_folder_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM task_folders WHERE id = new.task_folder_id
                AND workspace_id = new.workspace_id AND deleted = FALSE
        )
        BEGIN
            SELECT RAISE(ABORT, 'A saved view lists only a live folder of its workspace.');
        END;

        CREATE TRIGGER saved_task_views_folder_in_workspace_au
        BEFORE UPDATE OF task_folder_id ON saved_task_views
        WHEN new.task_folder_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM task_folders WHERE id = new.task_folder_id
                AND workspace_id = new.workspace_id AND deleted = FALSE
        )
        BEGIN
            SELECT RAISE(ABORT, 'A saved view lists only a live folder of its workspace.');
        END;

        CREATE TRIGGER saved_task_view_tags_in_workspace
        BEFORE INSERT ON saved_task_view_tags
        WHEN NOT EXISTS (
            SELECT 1 FROM tags JOIN saved_task_views ON saved_task_views.id = new.view_id
            WHERE tags.id = new.tag_id AND tags.entity = 'task'
                AND tags.workspace_id = saved_task_views.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'A saved view narrows only by a task tag of its workspace.');
        END;

        CREATE TRIGGER task_folders_saved_views_deleted
        AFTER UPDATE OF deleted ON task_folders
        WHEN new.deleted
        BEGIN
            DELETE FROM saved_task_views WHERE task_folder_id = new.id;
        END;",
    },
    // A saved task view also narrows by priority: a listed task has one of
    // the view's priorities, any when it names none.
    Migration {
        name: "saved_task_view_priorities",
        sql: "ALTER TABLE saved_task_views ADD COLUMN priorities TEXT NOT NULL DEFAULT '[]';",
    },
    // When the running turn of an agent's conversation started, so a
    // client can offer to stop it; none is running at upgrade. A script the
    // agent ran says how it ended in one tagged `outcome` instead of a
    // `result` and an `error` that might both be missing, so a script the
    // user stopped can say so.
    Migration {
        name: "agent_foreground_started_code_outcome",
        sql: "
        ALTER TABLE intelligence_agents ADD COLUMN foreground_started INTEGER;

        UPDATE intelligence_agent_messages
        SET message_content = json_set(
            json_remove(message_content, '$.result', '$.error'),
            '$.outcome',
            CASE
                WHEN message_content ->> '$.result' IS NOT NULL
                    THEN json_object('status', 'succeeded', 'result', message_content ->> '$.result')
                ELSE json_object('status', 'failed', 'error', coalesce(message_content ->> '$.error', ''))
            END)
        WHERE message_content ->> '$.kind' = 'codeExecution';",
    },
    // `hour:start` becomes `timer`, which fires every minute with the
    // minute too. A cue on the hour keeps firing once an hour: its code runs
    // only on the first minute the timer fires for in an hour, which is
    // :00, or the minute it woke at after the computer slept through :00.
    Migration {
        name: "hour_start_becomes_timer",
        sql: "
        UPDATE intelligence_agent_cues
        SET hook = 'timer',
            script = '(timer) => timer.since < timer.time - timer.minute * 60000 && ('
                || script || ')(timer)'
        WHERE hook = 'hour:start';

        UPDATE extension_hooks SET hook = 'timer' WHERE hook = 'hour:start';",
    },
    // A built-in endpoint says how many minutes its server keeps a prompt
    // cached, so an agent's thread idle for longer is rebuilt rather than
    // resent whole; a client's says nothing. Endpoints from before take
    // their provider's: 30 for OpenAI, 5 for any other. A column added to
    // rows already there can't be required by a CHECK, so triggers require
    // it of every built-in endpoint from now on.
    Migration {
        name: "endpoint_cache_minutes",
        sql: "
        ALTER TABLE intelligence_endpoints ADD COLUMN cache_minutes INTEGER
            CHECK (cache_minutes IS NULL OR (cache_minutes >= 1 AND client_id IS NULL));

        UPDATE intelligence_endpoints
        SET cache_minutes = CASE provider_identifier WHEN 'openai' THEN 30 ELSE 5 END
        WHERE client_id IS NULL;

        CREATE TRIGGER intelligence_endpoints_cache_minutes_insert
        BEFORE INSERT ON intelligence_endpoints
        WHEN new.client_id IS NULL AND new.cache_minutes IS NULL
        BEGIN
            SELECT RAISE(ABORT, 'A built-in endpoint says how long its cache lasts.');
        END;

        CREATE TRIGGER intelligence_endpoints_cache_minutes_update
        BEFORE UPDATE OF cache_minutes, client_id ON intelligence_endpoints
        WHEN new.client_id IS NULL AND new.cache_minutes IS NULL
        BEGIN
            SELECT RAISE(ABORT, 'A built-in endpoint says how long its cache lasts.');
        END;",
    },
    // An agent's message may reply to another message of its conversation,
    // which a client quotes above it; a reply to another agent's message is
    // refused.
    Migration {
        name: "agent_message_reply_to",
        sql: "
        ALTER TABLE intelligence_agent_messages ADD COLUMN reply_to INTEGER
            REFERENCES intelligence_agent_messages(id) ON DELETE SET NULL;

        CREATE TRIGGER intelligence_agent_messages_reply_to
        BEFORE INSERT ON intelligence_agent_messages
        WHEN new.reply_to IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM intelligence_agent_messages
            WHERE id = new.reply_to AND agent_id = new.agent_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'A reply answers a message of the same conversation.');
        END;",
    },
    // When a cue was last set, so a foreground cue that never fired can be
    // removed once it is a day old; cues from before count from the upgrade.
    Migration {
        name: "agent_cue_set_at",
        sql: "
        CREATE TABLE intelligence_agent_cues_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            hook TEXT NOT NULL,
            script TEXT NOT NULL,
            instructions TEXT NOT NULL,
            effort TEXT NOT NULL,
            set_at INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE (agent_id, name)
        );

        INSERT INTO intelligence_agent_cues_new
            (id, agent_id, name, hook, script, instructions, effort)
            SELECT id, agent_id, name, hook, script, instructions, effort
            FROM intelligence_agent_cues;

        DROP TABLE intelligence_agent_cues;
        ALTER TABLE intelligence_agent_cues_new RENAME TO intelligence_agent_cues;

        CREATE INDEX intelligence_agent_cues_hook ON intelligence_agent_cues(hook);",
    },
    // A forward was the user's text ending in a sentence naming the item,
    // which clients parsed back; it becomes a `forward` message carrying the
    // item. The sentence is the message's last paragraph, in the form sent
    // now (`Forwarded task [Fix login](cohesion://task/3/42).`, `\`, `[` and
    // `]` escaped in the title) or the one first sent (`Forwarded task #42
    // “Fix login” in workspace 3. Read it with the code tool: …`). A journal
    // entry takes its document's title, its day, as new forwards do. A
    // message that doesn't match one form exactly stays text.
    Migration {
        name: "agent_message_forwards",
        sql: r#"
        WITH RECURSIVE
        candidate(id, s) AS (
            SELECT id, char(10, 10) || (message_content ->> '$.text')
            FROM intelligence_agent_messages
            WHERE sender = 'user' AND message_content ->> '$.kind' = 'text'
                AND message_content ->> '$.text' LIKE '%Forwarded %'),
        -- Every "\n\nForwarded " in the text, to find the last.
        walk(id, s, p) AS (
            SELECT id, s, instr(s, char(10, 10) || 'Forwarded ') FROM candidate
            UNION ALL
            SELECT id, s, p + instr(substr(s, p + 1), char(10, 10) || 'Forwarded ') FROM walk
            WHERE instr(substr(s, p + 1), char(10, 10) || 'Forwarded ') > 0),
        split(id, note, ref) AS (
            SELECT id, CASE WHEN max(p) = 1 THEN '' ELSE substr(s, 3, max(p) - 3) END,
                substr(s, max(p) + 2)
            FROM walk GROUP BY id),
        nouns(id, note, ref, kind, noun) AS (
            SELECT id, note, ref, kind, noun FROM split
            JOIN (SELECT 'task' AS kind, 'task' AS noun
                  UNION ALL SELECT 'document', 'document'
                  UNION ALL SELECT 'journal', 'journal entry')
                ON substr(ref, 1, length('Forwarded ' || noun || ' ')) = 'Forwarded ' || noun || ' '),
        -- The current form, peeled from the end: ").", the id, "/", the
        -- workspace, "/", then the link's kind.
        l1(id, note, ref, kind, noun, body) AS (
            SELECT id, note, ref, kind, noun, substr(ref, 1, length(ref) - 2) FROM nouns
            WHERE substr(ref, 1, length('Forwarded ' || noun || ' [')) = 'Forwarded ' || noun || ' ['
                AND substr(ref, -2) = ').'),
        l2(id, note, ref, kind, noun, item, body) AS (
            SELECT id, note, ref, kind, noun, substr(body, length(rtrim(body, '0123456789')) + 1),
                rtrim(body, '0123456789')
            FROM l1),
        l3(id, note, ref, kind, noun, item, ws, body) AS (
            SELECT id, note, ref, kind, noun, item,
                substr(body, length(rtrim(substr(body, 1, length(body) - 1), '0123456789')) + 1,
                    length(body) - 1 - length(rtrim(substr(body, 1, length(body) - 1), '0123456789'))),
                rtrim(substr(body, 1, length(body) - 1), '0123456789')
            FROM l2 WHERE item <> '' AND substr(body, -1) = '/'),
        linked(id, note, kind, ws, item, title) AS (
            SELECT id, note, kind, CAST(ws AS INTEGER), CAST(item AS INTEGER),
                replace(replace(replace(replace(
                    substr(ref, length('Forwarded ' || noun || ' [') + 1,
                        length(ref) - length('Forwarded ' || noun || ' [')
                            - length('](cohesion://' || link || '/' || ws || '/' || item || ').')),
                    '\\', char(1)), '\[', '['), '\]', ']'), char(1), '\')
            FROM (SELECT *, CASE kind WHEN 'task' THEN 'task' ELSE 'document' END AS link FROM l3)
            WHERE ws <> ''
                AND substr(body, -length('](cohesion://' || link || '/'))
                    = '](cohesion://' || link || '/'),
        -- The first form: the id after its hash sign, then the sentence around the
        -- title, which names the same workspace and id twice.
        f1(id, note, ref, kind, noun, item) AS (
            SELECT id, note, ref, kind, noun,
                substr(ref, length('Forwarded ' || noun || ' #') + 1,
                    instr(substr(ref, length('Forwarded ' || noun || ' #') + 1), ' “') - 1)
            FROM nouns
            WHERE substr(ref, 1, length('Forwarded ' || noun || ' #')) = 'Forwarded ' || noun || ' #'
                AND instr(substr(ref, length('Forwarded ' || noun || ' #') + 1), ' “') > 0),
        f2(id, note, ref, kind, noun, item, ws, link) AS (
            SELECT id, note, ref, kind, noun, item,
                substr(rest, 1, length(rest) - length(ltrim(rest, '0123456789'))),
                CASE kind WHEN 'task' THEN 'task' ELSE 'document' END
            FROM (SELECT *, substr(ref, instr(ref, '” in workspace ') + length('” in workspace '))
                    AS rest FROM f1 WHERE instr(ref, '” in workspace ') > 0)
            WHERE item <> '' AND trim(item, '0123456789') = ''),
        older(id, note, kind, ws, item, title) AS (
            SELECT id, note, kind, CAST(ws AS INTEGER), CAST(item AS INTEGER),
                substr(ref, length('Forwarded ' || noun || ' #' || item || ' “') + 1,
                    length(ref) - length('Forwarded ' || noun || ' #' || item || ' “')
                        - length(suffix))
            FROM (SELECT *, '” in workspace ' || ws || '. Read it with the code tool: `cohesion.workspace('
                    || ws || ').' || link || '(' || item || ')`.' AS suffix FROM f2)
            WHERE ws <> ''
                AND substr(ref, -length(suffix)) = suffix
                AND length(ref) >= length('Forwarded ' || noun || ' #' || item || ' “') + length(suffix)),
        converted(id, note, kind, ws, item, title) AS (
            SELECT * FROM linked UNION ALL SELECT * FROM older)
        UPDATE intelligence_agent_messages
        SET message_content = json_object('kind', 'forward', 'note', c.note,
            'item', json_object('kind', c.kind, 'workspace_id', c.ws, 'id', c.item,
                'title', CASE WHEN c.kind = 'journal'
                    THEN coalesce((SELECT d.title FROM documents d
                                   WHERE d.id = c.item AND d.workspace_id = c.ws), c.title)
                    ELSE c.title END))
        FROM converted c
        WHERE intelligence_agent_messages.id = c.id;"#,
    },
    // `hour_start_becomes_timer` moved cues and extension hooks but not the
    // runs `hour:start` had already woken, which then failed to read.
    Migration {
        name: "hour_start_runs_become_timer",
        sql: "UPDATE intelligence_agent_runs SET hook = 'timer' WHERE hook = 'hour:start';",
    },
    // Agents ask each other: a question and its answer stand in both
    // agents' conversations without marking either unread, since neither
    // is said to the user; and a run says which agent's question woke it.
    Migration {
        name: "agent_asks",
        sql: "
        DROP TRIGGER intelligence_agent_messages_unread;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind')
            NOT IN ('codeExecution', 'documentation', 'memory', 'askedAgent', 'askedByAgent')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;

        ALTER TABLE intelligence_agent_runs ADD COLUMN sender_agent_id INTEGER
            REFERENCES intelligence_agents(id) ON DELETE SET NULL;",
    },
    // Who may reach the daemon over the network: when each client paired
    // and last connected, from where; the security settings (one row, today's
    // behaviour by default); the networks they may allow; and the security
    // activity.
    Migration {
        name: "security",
        sql: "
        ALTER TABLE clients ADD COLUMN created_at INTEGER;
        ALTER TABLE clients ADD COLUMN last_seen_at INTEGER;
        ALTER TABLE clients ADD COLUMN last_address TEXT;

        CREATE TABLE security_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            connect_from TEXT NOT NULL DEFAULT 'anywhere'
                CHECK (connect_from IN ('anywhere', 'allowedNetworks', 'thisComputer')),
            pair_from TEXT NOT NULL DEFAULT 'anywhere'
                CHECK (pair_from IN ('anywhere', 'allowedNetworks', 'thisComputer')),
            pairing_opened_by TEXT NOT NULL DEFAULT 'anyClient'
                CHECK (pairing_opened_by IN ('anyClient', 'localClients')),
            alert_new_client BOOLEAN NOT NULL DEFAULT FALSE,
            alert_new_address BOOLEAN NOT NULL DEFAULT FALSE,
            revoke_inactive_after_days INTEGER CHECK (revoke_inactive_after_days >= 1)
        );
        INSERT INTO security_settings (id) VALUES (1);

        CREATE TABLE allowed_networks (
            id INTEGER PRIMARY KEY,
            network TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE security_events (
            id INTEGER PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            kind TEXT NOT NULL,
            client_id INTEGER REFERENCES clients(id) ON DELETE SET NULL,
            client_name TEXT,
            address TEXT
        );
        CREATE INDEX security_events_created_at ON security_events(created_at);",
    },
    // The network controls go: behind a router that forwards the port and
    // loops LAN traffic back through it, every client looks like the router,
    // so allowing or blocking by address can't tell clients apart. What
    // they recorded goes with them. Pairing kept to this computer by address
    // stays kept to it, as clients on it.
    Migration {
        name: "security_without_networks",
        sql: "
        UPDATE security_settings SET pairing_opened_by = 'localClients'
            WHERE pair_from = 'thisComputer' OR connect_from = 'thisComputer';
        DROP TABLE allowed_networks;
        DELETE FROM security_events
            WHERE kind IN ('newAddress', 'connectionRefused', 'addressBlocked');
        ALTER TABLE security_settings DROP COLUMN connect_from;
        ALTER TABLE security_settings DROP COLUMN pair_from;
        ALTER TABLE security_settings DROP COLUMN alert_new_address;",
    },
    // Clients no longer offer models or connectors. A client's endpoint goes,
    // with its models and the tiers that used them; a client's connector
    // account is orphaned, as revoking the client did, described by the
    // client's name and keeping what was imported from it. Endpoints and
    // accounts lose `client_id`, so every endpoint is a built-in one with a
    // name and a cache lifetime, and every account has a description.
    // It is the first to run with foreign keys off, so it removes a client
    // endpoint's models and tiers itself. `legacy_alter_table` lets each new
    // table take its old one's name while triggers elsewhere still name it.
    Migration {
        name: "clients_offer_only_actors",
        sql: "
        DELETE FROM intelligence_defaults WHERE endpoint_model_id IN (
            SELECT endpoint_models.id FROM endpoint_models
            JOIN intelligence_endpoints ON intelligence_endpoints.id = endpoint_models.endpoint_id
            WHERE intelligence_endpoints.client_id IS NOT NULL
        );
        DELETE FROM endpoint_models WHERE endpoint_id IN (
            SELECT id FROM intelligence_endpoints WHERE client_id IS NOT NULL
        );
        DELETE FROM intelligence_endpoints WHERE client_id IS NOT NULL;
        UPDATE connector_accounts
        SET description = (SELECT client_name FROM clients WHERE id = connector_accounts.client_id)
        WHERE client_id IS NOT NULL;

        DROP TRIGGER clients_deleted_own_nothing;
        CREATE TRIGGER clients_deleted_own_nothing
        AFTER UPDATE OF deleted ON clients
        WHEN new.deleted
        BEGIN
            UPDATE actors SET deleted = TRUE WHERE client_id = new.id;
        END;

        CREATE TABLE connector_accounts_new (
            id INTEGER PRIMARY KEY,
            connector_id INTEGER NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
            extension_id INTEGER REFERENCES extensions(id) ON DELETE SET NULL,
            identifier TEXT,
            authorization TEXT,
            refresh TEXT,
            expires_at INTEGER,
            description TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK ((extension_id IS NULL) = (identifier IS NULL)
                AND (extension_id IS NULL) = (authorization IS NULL)
                AND (extension_id IS NOT NULL OR (refresh IS NULL AND expires_at IS NULL))),
            UNIQUE (extension_id, identifier)
        );
        INSERT INTO connector_accounts_new (id, connector_id, extension_id, identifier,
                authorization, refresh, expires_at, description, created_at)
            SELECT id, connector_id, extension_id, identifier, authorization, refresh,
                expires_at, description, created_at
            FROM connector_accounts;
        PRAGMA legacy_alter_table = ON;
        DROP TABLE connector_accounts;
        ALTER TABLE connector_accounts_new RENAME TO connector_accounts;

        CREATE TABLE intelligence_endpoints_new (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            provider_identifier TEXT NOT NULL,
            dialect TEXT NOT NULL,
            base_url TEXT NOT NULL,
            api_key TEXT,
            usage_url TEXT,
            account_id INTEGER REFERENCES connector_accounts(id) ON DELETE CASCADE,
            cache_minutes INTEGER NOT NULL CHECK (cache_minutes >= 1),
            CHECK (account_id IS NULL OR api_key IS NULL)
        );
        INSERT INTO intelligence_endpoints_new (id, name, provider_identifier, dialect,
                base_url, api_key, usage_url, account_id, cache_minutes)
            SELECT id, name, provider_identifier, dialect, base_url, api_key, usage_url,
                account_id, cache_minutes
            FROM intelligence_endpoints;
        DROP TABLE intelligence_endpoints;
        ALTER TABLE intelligence_endpoints_new RENAME TO intelligence_endpoints;
        PRAGMA legacy_alter_table = OFF;
        CREATE INDEX intelligence_endpoints_account ON intelligence_endpoints(account_id);

        CREATE TRIGGER connector_accounts_orphaned_endpoints
        AFTER UPDATE OF extension_id ON connector_accounts
        WHEN new.extension_id IS NULL
        BEGIN
            DELETE FROM intelligence_endpoints WHERE account_id = new.id;
        END;",
    },
    // Cues are background only: an agent hears from the actor tasks it
    // dispatches by following them, which wakes a fork of its conversation,
    // so the foreground cues that did that go, unfired. A task has at most
    // one follower.
    Migration {
        name: "actor_task_followers",
        sql: "
        DELETE FROM intelligence_agent_cues WHERE effort = 'foreground';

        CREATE TABLE actor_task_followers (
            id INTEGER PRIMARY KEY,
            task_id INTEGER NOT NULL UNIQUE REFERENCES actor_tasks(id) ON DELETE CASCADE,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            woken_for_attention BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE INDEX actor_task_followers_agent ON actor_task_followers(agent_id);",
    },
    // A task may say how much time or effort it takes, 1 to 5, and a saved
    // task view may narrow by it: a listed task has one of the view's sizes,
    // any task (sized or not) when it names none.
    Migration {
        name: "task_sizes",
        sql: "
        ALTER TABLE tasks ADD COLUMN size INTEGER CHECK (size BETWEEN 1 AND 5);
        ALTER TABLE saved_task_views ADD COLUMN sizes TEXT NOT NULL DEFAULT '[]';",
    },
    // Agents are organized into teams, each named once ignoring case; an
    // agent is on at most one, and deleting a team leaves its agents on none.
    Migration {
        name: "agent_teams",
        sql: "
        CREATE TABLE intelligence_agent_teams (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL UNIQUE COLLATE NOCASE
        );

        ALTER TABLE intelligence_agents ADD COLUMN team_id INTEGER
            REFERENCES intelligence_agent_teams(id) ON DELETE SET NULL;

        CREATE INDEX intelligence_agents_team ON intelligence_agents(team_id);",
    },
    // A memory is an agent's, a team's or every agent's: a team's goes with
    // the team. Every memory an agent sees opens each of its threads, each
    // scope capped, so none is found by word any more and the word index
    // goes. Ids are kept, as messages name memories by them.
    Migration {
        name: "team_memories",
        sql: "
        CREATE TABLE memories_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            team_id INTEGER REFERENCES intelligence_agent_teams(id) ON DELETE CASCADE,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            memory TEXT NOT NULL,
            CHECK (agent_id IS NULL OR team_id IS NULL)
        );
        INSERT INTO memories_new (id, agent_id, created_at, memory)
            SELECT id, agent_id, created_at, memory FROM memories;
        DROP TABLE memories;
        DROP TABLE memories_fts;
        ALTER TABLE memories_new RENAME TO memories;

        CREATE INDEX memories_agent ON memories(agent_id);
        CREATE INDEX memories_team ON memories(team_id);",
    },
    // What every model call together may cost over a rolling hour, day or
    // month before background work stops making model calls; a period
    // without a row has no limit, and none has one to start with.
    Migration {
        name: "intelligence_spend_limits",
        sql: "
        CREATE TABLE intelligence_spend_limits (
            id INTEGER PRIMARY KEY,
            period TEXT NOT NULL UNIQUE CHECK (period IN ('hour', 'day', 'month')),
            amount REAL NOT NULL CHECK (amount > 0)
        );",
    },
    // Which of an agent's conversation messages its live model thread
    // holds: where its extended context (the opening summary) starts, and
    // where its native context (the conversation as the thread holds it)
    // starts. Both or neither; agents start with none, and their next turn
    // starts a thread.
    Migration {
        name: "agent_thread_context",
        sql: "
        ALTER TABLE intelligence_agents ADD COLUMN thread_extended_from INTEGER;
        ALTER TABLE intelligence_agents ADD COLUMN thread_native_from INTEGER
            CHECK ((thread_native_from IS NULL) = (thread_extended_from IS NULL)
                AND (thread_native_from IS NULL OR thread_native_from >= thread_extended_from));",
    },
    // A built-in endpoint may offer a prompt cache longer than its default,
    // which an agent's requests ask for: how many minutes that one lasts,
    // or none. Endpoints from before take their provider's when they speak
    // its own API: an hour for Anthropic's, twelve for OpenAI's Responses.
    // Any other, or one signed in through an account, offers none.
    Migration {
        name: "endpoint_long_cache_minutes",
        sql: "
        ALTER TABLE intelligence_endpoints ADD COLUMN long_cache_minutes INTEGER
            CHECK (long_cache_minutes IS NULL OR long_cache_minutes >= 1);

        UPDATE intelligence_endpoints
        SET long_cache_minutes = CASE
            WHEN provider_identifier = 'anthropic' AND dialect = 'messages'
                AND base_url = 'https://api.anthropic.com/v1' THEN 60
            WHEN provider_identifier = 'openai' AND dialect = 'responses'
                AND base_url = 'https://api.openai.com/v1' THEN 720
        END
        WHERE account_id IS NULL;",
    },
    // An endpoint's long cache says what a write to it costs as well as how
    // long it lasts, so a custom endpoint can describe one: both as one
    // JSON value, or none. Endpoints that had one take their provider's
    // price: 1.6 times the short cache's write rate for Anthropic, 2 for
    // OpenAI.
    Migration {
        name: "endpoint_long_cache",
        sql: "
        ALTER TABLE intelligence_endpoints ADD COLUMN long_cache TEXT
            CHECK (long_cache IS NULL OR (json_valid(long_cache)
                AND json_type(long_cache, '$.minutes') = 'integer'
                AND json_extract(long_cache, '$.minutes') >= 1
                AND json_type(long_cache, '$.write_multiple') IN ('integer', 'real')
                AND json_extract(long_cache, '$.write_multiple') > 0));

        UPDATE intelligence_endpoints
        SET long_cache = json_object('minutes', long_cache_minutes, 'write_multiple',
            CASE provider_identifier WHEN 'anthropic' THEN 1.6 ELSE 2.0 END)
        WHERE long_cache_minutes IS NOT NULL;

        ALTER TABLE intelligence_endpoints DROP COLUMN long_cache_minutes;",
    },
    // Agents no longer detach runs: their table goes, with the activities
    // that showed them, and an activity is always an actor task's. The
    // table is rebuilt to drop `agent_id`, which a foreign key keeps
    // `DROP COLUMN` from removing, so its triggers and indexes are made
    // again, the kind check now only the task's NOT NULL.
    Migration {
        name: "remove_detached_runs",
        sql: "
        DROP TABLE intelligence_agent_detached_runs;

        CREATE TABLE activities_new (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            actor_task_id INTEGER NOT NULL REFERENCES actor_tasks(id) ON DELETE CASCADE,
            status TEXT NOT NULL DEFAULT 'running',
            progress TEXT,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            ended_at INTEGER
        );

        INSERT INTO activities_new (id, title, actor_task_id, status, progress, started_at,
                ended_at)
            SELECT id, title, actor_task_id, status, progress, started_at, ended_at
            FROM activities
            WHERE actor_task_id IS NOT NULL;

        DROP TABLE activities;
        ALTER TABLE activities_new RENAME TO activities;

        CREATE INDEX activities_started ON activities(ended_at, started_at);
        CREATE UNIQUE INDEX activities_one_open_actor_task ON activities(actor_task_id)
            WHERE ended_at IS NULL;

        CREATE TRIGGER activities_valid_insert BEFORE INSERT ON activities
        WHEN new.status NOT IN ('running', 'waiting', 'paused', 'succeeded', 'failed', 'cancelled')
            OR (new.ended_at IS NULL) <> (new.status IN ('running', 'waiting', 'paused'))
        BEGIN
            SELECT RAISE(ABORT, 'An activity has ended exactly when it has an outcome.');
        END;

        CREATE TRIGGER activities_valid_update BEFORE UPDATE ON activities
        WHEN new.status NOT IN ('running', 'waiting', 'paused', 'succeeded', 'failed', 'cancelled')
            OR (new.ended_at IS NULL) <> (new.status IN ('running', 'waiting', 'paused'))
        BEGIN
            SELECT RAISE(ABORT, 'An activity has ended exactly when it has an outcome.');
        END;

        CREATE TRIGGER activities_actor_task_unread AFTER UPDATE OF status ON activities
        WHEN new.status IN ('succeeded', 'failed')
        BEGIN
            UPDATE actor_tasks SET unread = TRUE WHERE id = new.actor_task_id;
        END;",
    },
    // A task's follower is a column of its own: the agent that last told it
    // something, which every stop on its own wakes. Nothing claims or ends a
    // follow any more, so the table and its once-per-attention flag go.
    Migration {
        name: "actor_task_follower_column",
        sql: "
        ALTER TABLE actor_tasks ADD COLUMN follower_agent_id INTEGER
            REFERENCES intelligence_agents(id) ON DELETE SET NULL;

        UPDATE actor_tasks SET follower_agent_id =
            (SELECT agent_id FROM actor_task_followers WHERE task_id = actor_tasks.id);

        DROP TABLE actor_task_followers;",
    },
    // An agent's notes to itself and the lines saying why it woke are its
    // own bookkeeping: like a script it ran, they leave it read.
    Migration {
        name: "agent_bookkeeping_messages",
        sql: "
        DROP TRIGGER intelligence_agent_messages_unread;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind') NOT IN
            ('codeExecution', 'documentation', 'memory', 'askedAgent', 'askedByAgent', 'note', 'woke')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // Every wake of an agent is a fork of its conversation, whose rights
    // follow from who started the chain: the user, or an event (a cue). A
    // task's follow keeps that, so a follow-up never gains rights. Grants,
    // which widened what a cue's run could do, go: such a fork asks.
    Migration {
        name: "initiative",
        sql: "
        ALTER TABLE actor_tasks ADD COLUMN follower_initiative TEXT NOT NULL DEFAULT 'user'
            CHECK (follower_initiative IN ('user', 'event'));

        DROP TABLE intelligence_agent_grants;",
    },
    // An agent asks with a choice now, never a proposal, so proposals go,
    // with the replies that quoted them.
    Migration {
        name: "proposals_removed",
        sql: "
        UPDATE intelligence_agent_messages SET reply_to = NULL
        WHERE reply_to IN (SELECT id FROM intelligence_agent_messages
            WHERE message_content ->> '$.kind' = 'proposal');

        DELETE FROM intelligence_agent_messages
        WHERE message_content ->> '$.kind' = 'proposal';",
    },
    // How long the cleanup keeps model threads, actor tasks and security
    // activity, and how many memories each scope keeps, were constants; a
    // singleton row holds them now, with the old values as defaults.
    Migration {
        name: "storage_settings",
        sql: "
        CREATE TABLE storage_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            thread_retention_days INTEGER NOT NULL DEFAULT 14
                CHECK (thread_retention_days >= 1),
            actor_task_retention_days INTEGER NOT NULL DEFAULT 7
                CHECK (actor_task_retention_days >= 1),
            security_event_retention_days INTEGER NOT NULL DEFAULT 90
                CHECK (security_event_retention_days >= 1),
            memory_limit INTEGER NOT NULL DEFAULT 20 CHECK (memory_limit >= 1)
        );
        INSERT INTO storage_settings (id) VALUES (1);",
    },
    // Contacts: the people a workspace keeps, each with a name, the email
    // addresses and phone numbers that reach them, and notes.
    Migration {
        name: "contacts",
        sql: "
        CREATE TABLE contacts (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            first_name TEXT NOT NULL,
            middle_name TEXT,
            last_name TEXT NOT NULL,
            methods TEXT NOT NULL DEFAULT '[]',
            notes TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX contacts_workspace ON contacts (workspace_id);",
    },
    // A contact's methods become rows of their own, and contacts and their
    // methods say when they were added and last changed. Changing a
    // contact's methods changes the contact.
    Migration {
        name: "contact_methods",
        sql: "
        CREATE TABLE contacts_rebuilt (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            first_name TEXT NOT NULL,
            middle_name TEXT,
            last_name TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch())
        );
        INSERT INTO contacts_rebuilt (id, workspace_id, deleted, first_name, middle_name,
                last_name, notes)
            SELECT id, workspace_id, deleted, first_name, middle_name, last_name, notes
            FROM contacts;

        CREATE TABLE contact_methods (
            id INTEGER PRIMARY KEY,
            contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('email', 'phone')),
            value TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch())
        );
        INSERT INTO contact_methods (contact_id, kind, value)
            SELECT c.id, m.value ->> '$.kind', m.value ->> '$.value'
            FROM contacts c, json_each(c.methods) m
            ORDER BY c.id, m.key;

        DROP TABLE contacts;
        ALTER TABLE contacts_rebuilt RENAME TO contacts;
        CREATE INDEX contacts_workspace ON contacts (workspace_id);
        CREATE INDEX contact_methods_contact ON contact_methods (contact_id);

        CREATE TRIGGER contact_methods_ai AFTER INSERT ON contact_methods
        BEGIN
            UPDATE contacts SET modified = unixepoch() WHERE id = new.contact_id;
        END;
        CREATE TRIGGER contact_methods_au AFTER UPDATE ON contact_methods
        BEGIN
            UPDATE contacts SET modified = unixepoch() WHERE id = new.contact_id;
        END;
        CREATE TRIGGER contact_methods_ad AFTER DELETE ON contact_methods
        BEGIN
            UPDATE contacts SET modified = unixepoch() WHERE id = old.contact_id;
        END;",
    },
    // A contact may have a photo: its type and base64 data, set together or
    // not at all.
    Migration {
        name: "contact_photos",
        sql: "
        ALTER TABLE contacts ADD COLUMN photo_media_type TEXT;
        ALTER TABLE contacts ADD COLUMN photo_data TEXT
            CHECK ((photo_data IS NULL) = (photo_media_type IS NULL));",
    },
    // The user's own profile, one row holding their name, never empty, and
    // their photo: its type and base64 data, set together or not at all.
    Migration {
        name: "profile",
        sql: "
        CREATE TABLE profile (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            name TEXT CHECK (name <> ''),
            photo_media_type TEXT,
            photo_data TEXT CHECK ((photo_data IS NULL) = (photo_media_type IS NULL))
        );
        INSERT INTO profile (id) VALUES (1);",
    },
    // A task may be assigned to the user or to one of their agents, never
    // both; deleting the agent leaves the task unassigned.
    Migration {
        name: "task_assignees",
        sql: "
        ALTER TABLE tasks ADD COLUMN assigned_to_user BOOLEAN NOT NULL DEFAULT FALSE;
        ALTER TABLE tasks ADD COLUMN assignee_agent_id INTEGER
            REFERENCES intelligence_agents(id) ON DELETE SET NULL
            CHECK (NOT (assigned_to_user AND assignee_agent_id IS NOT NULL));
        CREATE INDEX tasks_assignee_agent ON tasks(assignee_agent_id);",
    },
    // A task may instead be assigned to one of its workspace's live
    // contacts, and then to no one else; deleting the contact leaves the
    // task unassigned.
    Migration {
        name: "task_contact_assignees",
        sql: "
        ALTER TABLE tasks ADD COLUMN assignee_contact_id INTEGER
            REFERENCES contacts(id) ON DELETE SET NULL
            CHECK (assignee_contact_id IS NULL
                OR (NOT assigned_to_user AND assignee_agent_id IS NULL));
        CREATE INDEX tasks_assignee_contact ON tasks(assignee_contact_id);

        CREATE TRIGGER tasks_assignee_contact_bi BEFORE INSERT ON tasks
        WHEN new.assignee_contact_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM contacts WHERE contacts.id = new.assignee_contact_id
                AND contacts.workspace_id = new.workspace_id AND NOT contacts.deleted
        )
        BEGIN
            SELECT RAISE(ABORT, 'A task is assigned only to a live contact of its workspace.');
        END;
        CREATE TRIGGER tasks_assignee_contact_bu
        BEFORE UPDATE OF assignee_contact_id, workspace_id ON tasks
        WHEN new.assignee_contact_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM contacts WHERE contacts.id = new.assignee_contact_id
                AND contacts.workspace_id = new.workspace_id AND NOT contacts.deleted
        )
        BEGIN
            SELECT RAISE(ABORT, 'A task is assigned only to a live contact of its workspace.');
        END;

        CREATE TRIGGER contacts_deleted_assign_nothing
        AFTER UPDATE OF deleted ON contacts WHEN new.deleted AND NOT old.deleted
        BEGIN
            UPDATE tasks SET assignee_contact_id = NULL WHERE assignee_contact_id = new.id;
        END;",
    },
    // The line saying an agent's turn compressed its context into a fresh
    // thread is its own bookkeeping too: it leaves the agent read.
    Migration {
        name: "agent_compressed_context_messages",
        sql: "
        DROP TRIGGER intelligence_agent_messages_unread;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN json_extract(new.message_content, '$.kind') NOT IN
            ('codeExecution', 'documentation', 'memory', 'askedAgent', 'askedByAgent', 'note', 'woke',
             'compressedContext')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // How many tokens an agent's live model thread held at its latest
    // request. Only a live thread has a count.
    Migration {
        name: "agent_context_tokens",
        sql: "
        ALTER TABLE intelligence_agents ADD COLUMN context_tokens INTEGER
            CHECK (context_tokens IS NULL
                OR (context_tokens >= 0 AND thread_native_from IS NOT NULL));",
    },
    // An actor task can be resolved: the user is done with it, and only a
    // resolved task is ever cleaned up.
    Migration {
        name: "actor_task_resolved",
        sql: "
        DROP TRIGGER actor_tasks_valid_insert;
        DROP TRIGGER actor_tasks_valid_update;

        CREATE TRIGGER actor_tasks_valid_insert BEFORE INSERT ON actor_tasks
        WHEN new.status NOT IN ('working', 'needsAttention', 'idle', 'failed', 'resolved')
        BEGIN
            SELECT RAISE(ABORT, 'An actor task is working, needs attention, idle, failed or resolved.');
        END;

        CREATE TRIGGER actor_tasks_valid_update BEFORE UPDATE OF status ON actor_tasks
        WHEN new.status NOT IN ('working', 'needsAttention', 'idle', 'failed', 'resolved')
        BEGIN
            SELECT RAISE(ABORT, 'An actor task is working, needs attention, idle, failed or resolved.');
        END;",
    },
    // Where each agent's model threads stood when their last turns ended,
    // so a restarted daemon carries a thread on while its provider still
    // holds the prompt cached. One per agent and source; a mark goes with
    // its agent, its thread and its model.
    Migration {
        name: "agent_thread_marks",
        sql: "
        CREATE TABLE agent_thread_marks (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            source TEXT NOT NULL CHECK (source IN ('conversation', 'opening')),
            thread_id INTEGER NOT NULL REFERENCES intelligence_threads(id) ON DELETE CASCADE,
            items INTEGER NOT NULL CHECK (items >= 0),
            endpoint_model_id INTEGER NOT NULL REFERENCES endpoint_models(id) ON DELETE CASCADE,
            heard_through INTEGER NOT NULL,
            memories TEXT NOT NULL,
            last_call_at INTEGER NOT NULL,
            prompt_tokens INTEGER NOT NULL CHECK (prompt_tokens >= 0),
            cache_seconds INTEGER NOT NULL CHECK (cache_seconds > 0),
            UNIQUE (agent_id, source)
        );

        CREATE INDEX agent_thread_marks_thread ON agent_thread_marks(thread_id);

        CREATE INDEX agent_thread_marks_endpoint_model
            ON agent_thread_marks(endpoint_model_id);",
    },
    // A thread mark says which messages its thread holds, so the agent's
    // own copy of the thread's bounds goes: the mark's `extended_from`
    // takes the conversation thread's (an agent with no conversation mark
    // has no thread to carry on), and the opening marks, which never read
    // one, are forgotten for the next fork to write again. An agent's
    // context tokens are rebuilt without the check tying them to the bounds
    // (the count is dropped with them; the next request restores it).
    Migration {
        name: "agent_thread_mark_extended_from",
        sql: "
        ALTER TABLE agent_thread_marks ADD COLUMN extended_from INTEGER NOT NULL DEFAULT 0;

        DELETE FROM agent_thread_marks
        WHERE source = 'opening'
            OR agent_id IN (
                SELECT id FROM intelligence_agents WHERE thread_extended_from IS NULL);

        UPDATE agent_thread_marks
        SET extended_from = (
            SELECT thread_extended_from FROM intelligence_agents
            WHERE intelligence_agents.id = agent_thread_marks.agent_id);

        ALTER TABLE intelligence_agents DROP COLUMN context_tokens;
        ALTER TABLE intelligence_agents DROP COLUMN thread_native_from;
        ALTER TABLE intelligence_agents DROP COLUMN thread_extended_from;
        ALTER TABLE intelligence_agents ADD COLUMN context_tokens INTEGER
            CHECK (context_tokens IS NULL OR context_tokens >= 0);",
    },
    // A failed `completeRefresh` flags the account, so clients can offer to
    // sign it in again; a new credential, from a refresh or a sign-in,
    // clears the flag.
    Migration {
        name: "connector_account_credential_failed_at",
        sql: "ALTER TABLE connector_accounts ADD COLUMN credential_failed_at INTEGER;",
    },
    // Rewind: a copy of a task's title, description and status, and of a
    // document's title and contents, each time one changes. Triggers keep
    // them so every write is covered. A trigger can't see the command, so
    // `rewind_writer` holds who that command writes as (the worker sets it)
    // and whether it is restoring. Versions stay at least two minutes apart:
    // an edit within two minutes of the latest version's start joins it, and
    // names its own author. A restore or a deletion never joins, and an
    // `earlier` or `deleted` version is never joined, so a restore can be
    // undone and a deleted item keeps its text.
    // The first change to a row with no versions (one older than this
    // migration, or whose versions have expired) first keeps what it held,
    // as an `earlier` version. An author's name is copied, since agents
    // and extensions are removed outright. A deleted journal could come back beside a new entry
    // for its day, so the schema now holds one live journal a day.
    Migration {
        name: "rewind",
        sql: "
        ALTER TABLE storage_settings ADD COLUMN rewind_retention_days INTEGER NOT NULL DEFAULT 7
            CHECK (rewind_retention_days >= 1);

        CREATE UNIQUE INDEX documents_live_journal ON documents(workspace_id, journal_date)
            WHERE deleted = 0 AND journal_date IS NOT NULL;

        CREATE TABLE rewind_writer (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            author TEXT NOT NULL DEFAULT 'user' CHECK (author IN ('user', 'agent', 'extension', 'script')),
            agent_id INTEGER,
            extension TEXT,
            restoring BOOLEAN NOT NULL DEFAULT FALSE
        );
        INSERT INTO rewind_writer (id) VALUES (1);

        CREATE VIEW rewind_author AS
            SELECT w.author AS author,
                CASE w.author
                    WHEN 'agent' THEN COALESCE(a.agent_name, 'Agent')
                    WHEN 'extension' THEN COALESCE(e.name, w.extension)
                END AS author_name,
                w.restoring AS restoring
            FROM rewind_writer w
            LEFT JOIN intelligence_agents a ON a.id = w.agent_id
            LEFT JOIN extensions e ON e.script_identifier = w.extension;

        CREATE TABLE task_versions (
            id INTEGER PRIMARY KEY,
            task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL,
            change TEXT NOT NULL
                CHECK (change IN ('created', 'edited', 'deleted', 'restored', 'earlier')),
            author TEXT NOT NULL CHECK (author IN ('user', 'agent', 'extension', 'script', 'unknown')),
            author_name TEXT,
            created INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK ((author IN ('agent', 'extension')) = (author_name IS NOT NULL))
        );
        CREATE INDEX task_versions_task ON task_versions(task_id, id);
        CREATE INDEX task_versions_modified ON task_versions(modified);

        CREATE TRIGGER tasks_version_ai AFTER INSERT ON tasks
        BEGIN
            INSERT INTO task_versions (task_id, workspace_id, title, description, status, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.description, new.status, 'created', author, author_name
                FROM rewind_author;
        END;

        CREATE TRIGGER tasks_version_au
        AFTER UPDATE OF title, description, status, deleted ON tasks
        WHEN old.title IS NOT new.title OR old.description IS NOT new.description
            OR old.status IS NOT new.status OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO task_versions (task_id, workspace_id, title, description, status, change, author)
                SELECT old.id, old.workspace_id, old.title, old.description, old.status, 'earlier', 'unknown'
                WHERE NOT EXISTS (SELECT 1 FROM task_versions WHERE task_id = old.id);

            INSERT INTO task_versions (task_id, workspace_id, title, description, status, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.description, new.status,
                    CASE WHEN r.restoring THEN 'restored'
                        WHEN new.deleted AND NOT old.deleted THEN 'deleted'
                        ELSE 'edited' END,
                    r.author, r.author_name
                FROM rewind_author r
                WHERE new.deleted IS NOT old.deleted OR r.restoring OR NOT EXISTS (
                    SELECT 1 FROM task_versions v
                    WHERE v.id = (SELECT MAX(id) FROM task_versions WHERE task_id = new.id)
                        AND v.change IN ('created', 'edited', 'restored')
                        AND v.created > unixepoch() - 120);

            UPDATE task_versions
            SET title = new.title, description = new.description, status = new.status,
                change = CASE change WHEN 'restored' THEN 'edited' ELSE change END,
                author = (SELECT author FROM rewind_author),
                author_name = (SELECT author_name FROM rewind_author),
                modified = unixepoch()
            WHERE id = (SELECT MAX(id) FROM task_versions WHERE task_id = new.id)
                AND change IN ('created', 'edited', 'restored')
                AND created > unixepoch() - 120
                AND new.deleted IS old.deleted
                AND NOT (SELECT restoring FROM rewind_author);
        END;

        CREATE TABLE document_versions (
            id INTEGER PRIMARY KEY,
            document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            contents TEXT NOT NULL,
            change TEXT NOT NULL
                CHECK (change IN ('created', 'edited', 'deleted', 'restored', 'earlier')),
            author TEXT NOT NULL CHECK (author IN ('user', 'agent', 'extension', 'script', 'unknown')),
            author_name TEXT,
            created INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK ((author IN ('agent', 'extension')) = (author_name IS NOT NULL))
        );
        CREATE INDEX document_versions_document ON document_versions(document_id, id);
        CREATE INDEX document_versions_modified ON document_versions(modified);

        CREATE TRIGGER documents_version_ai AFTER INSERT ON documents
        BEGIN
            INSERT INTO document_versions (document_id, workspace_id, title, contents, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.contents, 'created', author, author_name
                FROM rewind_author;
        END;

        CREATE TRIGGER documents_version_au
        AFTER UPDATE OF title, contents, deleted ON documents
        WHEN old.title IS NOT new.title OR old.contents IS NOT new.contents
            OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO document_versions (document_id, workspace_id, title, contents, change, author)
                SELECT old.id, old.workspace_id, old.title, old.contents, 'earlier', 'unknown'
                WHERE NOT EXISTS (SELECT 1 FROM document_versions WHERE document_id = old.id);

            INSERT INTO document_versions (document_id, workspace_id, title, contents, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.contents,
                    CASE WHEN r.restoring THEN 'restored'
                        WHEN new.deleted AND NOT old.deleted THEN 'deleted'
                        ELSE 'edited' END,
                    r.author, r.author_name
                FROM rewind_author r
                WHERE new.deleted IS NOT old.deleted OR r.restoring OR NOT EXISTS (
                    SELECT 1 FROM document_versions v
                    WHERE v.id = (SELECT MAX(id) FROM document_versions WHERE document_id = new.id)
                        AND v.change IN ('created', 'edited', 'restored')
                        AND v.created > unixepoch() - 120);

            UPDATE document_versions
            SET title = new.title, contents = new.contents,
                change = CASE change WHEN 'restored' THEN 'edited' ELSE change END,
                author = (SELECT author FROM rewind_author),
                author_name = (SELECT author_name FROM rewind_author),
                modified = unixepoch()
            WHERE id = (SELECT MAX(id) FROM document_versions WHERE document_id = new.id)
                AND change IN ('created', 'edited', 'restored')
                AND created > unixepoch() - 120
                AND new.deleted IS old.deleted
                AND NOT (SELECT restoring FROM rewind_author);
        END;",
    },
    // Thread messages leave JSON for columns (a kind by number and what each
    // kind sets), a system prompt is stored once however many threads share
    // it, and a thread may carry on another's messages rather than copy them.
    // Threads already forked keep the rows they copied, as roots, and age out
    // as they did. `description` repeated a user message's text and read as
    // nothing else, and `search_eligible` and `agent_thread` meant nothing
    // once Ask went.
    Migration {
        name: "thread_columns_prompts_and_forks",
        sql: "
        CREATE TABLE thread_prompts (
            id INTEGER PRIMARY KEY,
            prompt TEXT NOT NULL UNIQUE
        );
        INSERT OR IGNORE INTO thread_prompts (prompt)
            SELECT system_prompt FROM intelligence_threads;

        CREATE TABLE intelligence_threads_new (
            id INTEGER PRIMARY KEY,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            prompt_id INTEGER NOT NULL REFERENCES thread_prompts(id),
            parent_thread_id INTEGER REFERENCES intelligence_threads(id) ON DELETE CASCADE,
            inherited_through_id INTEGER,
            inherited_items INTEGER,
            remote_id TEXT,
            remote_cursor INTEGER,
            CHECK ((parent_thread_id IS NULL) = (inherited_through_id IS NULL)
                AND (parent_thread_id IS NULL) = (inherited_items IS NULL)),
            CHECK ((remote_id IS NULL) = (remote_cursor IS NULL))
        );
        INSERT INTO intelligence_threads_new (id, created_at, prompt_id, remote_id, remote_cursor)
            SELECT t.id, t.created_at, p.id,
                CASE WHEN t.remote_cursor IS NOT NULL THEN t.remote_id END,
                CASE WHEN t.remote_id IS NOT NULL THEN t.remote_cursor END
            FROM intelligence_threads t
            JOIN thread_prompts p ON p.prompt = t.system_prompt;

        CREATE TABLE intelligence_thread_messages_new (
            id INTEGER PRIMARY KEY,
            thread_id INTEGER NOT NULL REFERENCES intelligence_threads(id) ON DELETE CASCADE,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 5),
            text TEXT,
            signature TEXT,
            in_content BOOLEAN,
            redacted_trace TEXT,
            call_identifier TEXT,
            tool_name TEXT,
            arguments TEXT,
            succeeded BOOLEAN,
            CHECK (text IS NOT NULL OR kind IN (3, 4)),
            CHECK (kind = 2 OR (signature IS NULL AND in_content IS NULL)),
            CHECK ((redacted_trace IS NOT NULL) = (kind = 3)),
            CHECK ((call_identifier IS NOT NULL) = (kind IN (4, 5))),
            CHECK ((tool_name IS NOT NULL) = (kind = 4)),
            CHECK ((arguments IS NOT NULL) = (kind = 4)),
            CHECK ((succeeded IS NOT NULL) = (kind = 5))
        );
        INSERT INTO intelligence_thread_messages_new (id, thread_id, created_at, kind, text,
                signature, in_content, redacted_trace, call_identifier, tool_name, arguments,
                succeeded)
            SELECT id, thread_id, created_at,
                CASE kind
                    WHEN 'user-message' THEN 0
                    WHEN 'assistant-message' THEN 1
                    WHEN 'thinking' THEN
                        CASE WHEN payload ->> '$.Thinking.Redacted.trace' IS NOT NULL THEN 3 ELSE 2 END
                    WHEN 'tool-call-request' THEN 4
                    ELSE 5 END,
                CASE kind
                    WHEN 'user-message' THEN coalesce(payload ->> '$.UserMessage.text', '')
                    WHEN 'assistant-message' THEN coalesce(payload ->> '$.AssistantMessage.text', '')
                    WHEN 'thinking' THEN
                        CASE WHEN payload ->> '$.Thinking.Redacted.trace' IS NOT NULL
                            THEN payload ->> '$.Thinking.Redacted.summary'
                            ELSE coalesce(payload ->> '$.Thinking.Standard.thought', '') END
                    WHEN 'tool-call-request' THEN NULL
                    ELSE coalesce(payload ->> '$.ToolCallResponse.response.Success',
                        payload ->> '$.ToolCallResponse.response.Failure', '') END,
                CASE WHEN kind = 'thinking' THEN payload ->> '$.Thinking.Standard.signature' END,
                CASE WHEN kind = 'thinking' AND payload ->> '$.Thinking.Redacted.trace' IS NULL
                    THEN coalesce(payload ->> '$.Thinking.Standard.in_content', 0) END,
                CASE WHEN kind = 'thinking' THEN payload ->> '$.Thinking.Redacted.trace' END,
                CASE kind
                    WHEN 'tool-call-request'
                        THEN coalesce(payload ->> '$.ToolCallRequest.call_identifier', '')
                    WHEN 'tool-call-response'
                        THEN coalesce(payload ->> '$.ToolCallResponse.call_identifier', '') END,
                CASE WHEN kind = 'tool-call-request'
                    THEN coalesce(payload ->> '$.ToolCallRequest.tool_name', '') END,
                CASE WHEN kind = 'tool-call-request'
                    THEN coalesce(payload ->> '$.ToolCallRequest.arguments', '{}') END,
                CASE WHEN kind NOT IN ('user-message', 'assistant-message', 'thinking',
                        'tool-call-request')
                    THEN payload ->> '$.ToolCallResponse.response.Failure' IS NULL END
            FROM intelligence_thread_messages
            WHERE json_valid(payload);

        CREATE TABLE intelligence_thread_images (
            id INTEGER PRIMARY KEY,
            message_id INTEGER NOT NULL
                REFERENCES intelligence_thread_messages(id) ON DELETE CASCADE,
            media_type TEXT NOT NULL,
            data BLOB NOT NULL
        );
        INSERT INTO intelligence_thread_images (message_id, media_type, data)
            SELECT m.id, image.value ->> '$.media_type',
                unbase64(image.value ->> '$.data')
            FROM intelligence_thread_messages m, json_each(m.payload, '$.UserMessage.images') image
            WHERE m.kind = 'user-message' AND json_valid(m.payload);
        CREATE INDEX intelligence_thread_images_message
            ON intelligence_thread_images(message_id);

        PRAGMA legacy_alter_table = ON;
        DROP TABLE intelligence_thread_messages;
        ALTER TABLE intelligence_thread_messages_new RENAME TO intelligence_thread_messages;
        DROP TABLE intelligence_threads;
        ALTER TABLE intelligence_threads_new RENAME TO intelligence_threads;
        PRAGMA legacy_alter_table = OFF;

        CREATE INDEX intelligence_thread_messages_thread
            ON intelligence_thread_messages(thread_id, id);
        CREATE INDEX intelligence_threads_parent
            ON intelligence_threads(parent_thread_id) WHERE parent_thread_id IS NOT NULL;",
    },
    // Agent and actor messages leave JSON for columns as thread messages did;
    // only an actor's nested requests and answers, read and written whole, stay
    // JSON in `details`, and a choice's list of options in `options`. Images
    // from either conversation move to one table of bytes, whose count replaces
    // a column that repeated it.
    Migration {
        name: "message_columns_and_attachments",
        sql: "
        CREATE TABLE intelligence_agent_messages_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            run_id INTEGER REFERENCES intelligence_agent_runs(id) ON DELETE SET NULL,
            run_description TEXT,
            sent_at INTEGER NOT NULL DEFAULT (unixepoch()),
            sender TEXT NOT NULL,
            reaction TEXT,
            reacted_at INTEGER,
            reply_to INTEGER REFERENCES intelligence_agent_messages(id) ON DELETE SET NULL,
            kind TEXT NOT NULL CHECK (kind IN ('text', 'codeExecution', 'error', 'documentation',
                'choice', 'memory', 'forward', 'askedAgent', 'askedByAgent', 'note', 'woke',
                'compressedContext')),
            text TEXT,
            title TEXT,
            code TEXT,
            outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'cancelled')),
            outcome_text TEXT,
            options TEXT,
            context TEXT,
            selected INTEGER,
            change TEXT CHECK (change IN ('remembered', 'forgotten')),
            item_kind TEXT,
            item_workspace_id INTEGER,
            item_id INTEGER,
            peer_id INTEGER,
            peer_name TEXT,
            reply TEXT CHECK (reply IN ('waiting', 'answered', 'unanswered')),
            reply_text TEXT,
            wake TEXT CHECK (wake IN ('cue', 'actorTask', 'assignedTask')),
            cue_name TEXT,
            hook TEXT,
            actor_task_id INTEGER,
            stop TEXT,
            reason TEXT,
            CHECK ((text IS NOT NULL) = (kind NOT IN ('codeExecution', 'woke', 'compressedContext'))),
            CHECK ((outcome IS NOT NULL) = (kind = 'codeExecution')),
            CHECK ((code IS NOT NULL) = (kind = 'codeExecution')),
            CHECK ((options IS NOT NULL) = (kind = 'choice')),
            CHECK ((change IS NOT NULL) = (kind = 'memory')),
            CHECK ((item_kind IS NOT NULL) = (kind = 'forward')),
            CHECK ((peer_id IS NOT NULL) = (kind IN ('askedAgent', 'askedByAgent'))),
            CHECK ((reply IS NOT NULL) = (kind IN ('askedAgent', 'askedByAgent'))),
            CHECK ((wake IS NOT NULL) = (kind = 'woke'))
        );
        INSERT INTO intelligence_agent_messages_new (id, agent_id, run_id, run_description, sent_at,
                sender, reaction, reacted_at, reply_to, kind, text, title, code, outcome,
                outcome_text, options, context, selected, change, item_kind, item_workspace_id,
                item_id, peer_id, peer_name, reply, reply_text, wake, cue_name, hook,
                actor_task_id, stop, reason)
            SELECT id, agent_id, run_id, run_description, sent_at, sender, reaction, reacted_at,
                reply_to, message_content ->> '$.kind',
                CASE message_content ->> '$.kind'
                    WHEN 'text' THEN message_content ->> '$.text'
                    WHEN 'error' THEN message_content ->> '$.error'
                    WHEN 'documentation' THEN message_content ->> '$.module'
                    WHEN 'choice' THEN message_content ->> '$.question'
                    WHEN 'memory' THEN message_content ->> '$.memory'
                    WHEN 'forward' THEN message_content ->> '$.note'
                    WHEN 'askedAgent' THEN message_content ->> '$.question'
                    WHEN 'askedByAgent' THEN message_content ->> '$.question'
                    WHEN 'note' THEN message_content ->> '$.note' END,
                coalesce(message_content ->> '$.title', message_content ->> '$.item.title',
                    message_content ->> '$.by.title'),
                message_content ->> '$.code',
                message_content ->> '$.outcome.status',
                coalesce(message_content ->> '$.outcome.result',
                    message_content ->> '$.outcome.error'),
                message_content -> '$.options',
                message_content ->> '$.context',
                message_content ->> '$.selected',
                message_content ->> '$.change',
                message_content ->> '$.item.kind',
                coalesce(message_content ->> '$.item.workspace_id',
                    message_content ->> '$.by.workspace_id'),
                coalesce(message_content ->> '$.item.id', message_content ->> '$.by.task_id'),
                message_content ->> '$.agent.id',
                message_content ->> '$.agent.name',
                message_content ->> '$.reply.status',
                coalesce(message_content ->> '$.reply.text', message_content ->> '$.reply.reason'),
                message_content ->> '$.by.kind',
                message_content ->> '$.by.cue_name',
                message_content ->> '$.by.hook',
                message_content ->> '$.by.actor_task_id',
                message_content ->> '$.by.stop',
                message_content ->> '$.reason'
            FROM intelligence_agent_messages;

        CREATE TABLE actor_task_messages_new (
            id INTEGER PRIMARY KEY,
            task_id INTEGER NOT NULL REFERENCES actor_tasks(id) ON DELETE CASCADE,
            sent_at INTEGER NOT NULL DEFAULT (unixepoch()),
            sender TEXT NOT NULL CHECK (sender IN ('actor', 'user')),
            kind TEXT NOT NULL,
            text TEXT,
            ended_at INTEGER,
            tool TEXT,
            category TEXT,
            request_id INTEGER,
            decision TEXT,
            approved BOOLEAN,
            details TEXT,
            CHECK (sender = 'actor' AND kind IN ('text', 'thinking', 'toolUse', 'question',
                    'permission', 'plan')
                OR sender = 'user' AND kind IN ('text', 'answers', 'permission', 'plan')),
            CHECK ((request_id IS NOT NULL) = (sender = 'user' AND kind <> 'text')),
            CHECK ((tool IS NOT NULL) = (kind = 'toolUse')),
            CHECK ((category IS NOT NULL) = (kind = 'toolUse')),
            CHECK ((decision IS NOT NULL) = (sender = 'user' AND kind = 'permission')),
            CHECK ((approved IS NOT NULL) = (sender = 'user' AND kind = 'plan'))
        );
        INSERT INTO actor_task_messages_new (id, task_id, sent_at, sender, kind, text, ended_at,
                tool, category, request_id, decision, approved, details)
            SELECT id, task_id, sent_at, content ->> '$.sender', content ->> '$.kind',
                CASE content ->> '$.kind'
                    WHEN 'text' THEN content ->> '$.text'
                    WHEN 'thinking' THEN content ->> '$.text'
                    WHEN 'toolUse' THEN content ->> '$.summary'
                    WHEN 'plan' THEN coalesce(content ->> '$.plan', content ->> '$.feedback')
                    WHEN 'permission' THEN content ->> '$.message' END,
                content ->> '$.ended_at',
                content ->> '$.tool',
                CASE WHEN content ->> '$.kind' = 'toolUse'
                    THEN coalesce(content ->> '$.category', 'other') END,
                content ->> '$.request_id',
                content ->> '$.decision',
                content ->> '$.approved',
                CASE
                    WHEN content ->> '$.kind' = 'question' THEN content -> '$.questions'
                    WHEN content ->> '$.kind' = 'answers' THEN content -> '$.answers'
                    WHEN content ->> '$.kind' = 'permission' AND content ->> '$.sender' = 'actor'
                        THEN json_remove(content, '$.sender', '$.kind') END
            FROM actor_task_messages;

        CREATE TABLE attachments (
            id INTEGER PRIMARY KEY,
            actor_task_message_id INTEGER REFERENCES actor_task_messages(id) ON DELETE CASCADE,
            agent_message_id INTEGER REFERENCES intelligence_agent_messages(id) ON DELETE CASCADE,
            media_type TEXT NOT NULL,
            data BLOB NOT NULL,
            CHECK ((actor_task_message_id IS NULL) <> (agent_message_id IS NULL))
        );
        INSERT INTO attachments (actor_task_message_id, media_type, data)
            SELECT message_id, media_type, unbase64(data) FROM actor_task_attachments
            WHERE unbase64(data) IS NOT NULL ORDER BY id;
        INSERT INTO attachments (agent_message_id, media_type, data)
            SELECT message_id, media_type, unbase64(data) FROM intelligence_agent_attachments
            WHERE unbase64(data) IS NOT NULL ORDER BY id;

        PRAGMA legacy_alter_table = ON;
        DROP TABLE actor_task_attachments;
        DROP TABLE intelligence_agent_attachments;
        DROP TABLE intelligence_agent_messages;
        ALTER TABLE intelligence_agent_messages_new RENAME TO intelligence_agent_messages;
        DROP TABLE actor_task_messages;
        ALTER TABLE actor_task_messages_new RENAME TO actor_task_messages;
        PRAGMA legacy_alter_table = OFF;

        CREATE INDEX attachments_actor_task_message ON attachments(actor_task_message_id)
            WHERE actor_task_message_id IS NOT NULL;
        CREATE INDEX attachments_agent_message ON attachments(agent_message_id)
            WHERE agent_message_id IS NOT NULL;
        CREATE INDEX intelligence_agent_messages_agent ON intelligence_agent_messages(agent_id, id);
        CREATE INDEX intelligence_agent_messages_run ON intelligence_agent_messages(run_id)
            WHERE run_id IS NOT NULL;
        CREATE INDEX intelligence_agent_messages_reply_to ON intelligence_agent_messages(reply_to)
            WHERE reply_to IS NOT NULL;
        CREATE INDEX actor_task_messages_task ON actor_task_messages(task_id, id);
        CREATE UNIQUE INDEX actor_task_messages_one_response ON actor_task_messages(request_id)
            WHERE request_id IS NOT NULL;

        CREATE TRIGGER actor_task_messages_search_ai AFTER INSERT ON actor_task_messages
        WHEN new.kind = 'text'
        BEGIN
            INSERT INTO actor_task_messages_fts (rowid, text) VALUES (new.id, new.text);
        END;
        CREATE TRIGGER actor_task_messages_search_ad AFTER DELETE ON actor_task_messages
        BEGIN
            DELETE FROM actor_task_messages_fts WHERE rowid = old.id;
        END;
        CREATE TRIGGER actor_task_messages_search_au AFTER UPDATE OF text ON actor_task_messages
        BEGIN
            DELETE FROM actor_task_messages_fts WHERE rowid = old.id;
            INSERT INTO actor_task_messages_fts (rowid, text)
                SELECT new.id, new.text WHERE new.kind = 'text';
        END;
        CREATE TRIGGER intelligence_agent_messages_reply_to
        BEFORE INSERT ON intelligence_agent_messages
        WHEN new.reply_to IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM intelligence_agent_messages
            WHERE id = new.reply_to AND agent_id = new.agent_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'A reply answers a message of the same conversation.');
        END;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN new.kind NOT IN ('codeExecution', 'documentation', 'memory', 'askedAgent',
            'askedByAgent', 'note', 'woke', 'compressedContext')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // Rewind's two tables and their near-identical triggers become one table,
    // with a real foreign key for each kind of item, which also holds a version
    // to its item's workspace, and one write rule: every
    // item's trigger writes through the `version_writes` view, whose trigger
    // holds when an edit joins the latest version and when it keeps its own.
    Migration {
        name: "unified_rewind",
        sql: "
        CREATE UNIQUE INDEX tasks_workspace_id ON tasks(workspace_id, id);
        CREATE UNIQUE INDEX documents_workspace_id ON documents(workspace_id, id);
        CREATE TABLE versions (
            id INTEGER PRIMARY KEY,
            task_id INTEGER,
            document_id INTEGER,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT,
            change TEXT NOT NULL
                CHECK (change IN ('created', 'edited', 'deleted', 'restored', 'earlier')),
            author TEXT NOT NULL CHECK (author IN ('user', 'agent', 'extension', 'script', 'unknown')),
            author_name TEXT,
            created INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            CHECK ((task_id IS NULL) <> (document_id IS NULL)),
            CHECK ((status IS NOT NULL) = (task_id IS NOT NULL)),
            CHECK ((author IN ('agent', 'extension')) = (author_name IS NOT NULL)),
            FOREIGN KEY (workspace_id, task_id) REFERENCES tasks(workspace_id, id)
                ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, document_id) REFERENCES documents(workspace_id, id)
                ON DELETE CASCADE
        );
        INSERT INTO versions (task_id, workspace_id, title, body, status, change, author,
                author_name, created, modified)
            SELECT task_id, workspace_id, title, description, status, change, author, author_name,
                created, modified
            FROM task_versions ORDER BY id;
        INSERT INTO versions (document_id, workspace_id, title, body, change, author, author_name,
                created, modified)
            SELECT document_id, workspace_id, title, contents, change, author, author_name,
                created, modified
            FROM document_versions ORDER BY id;

        DROP TRIGGER tasks_version_ai;
        DROP TRIGGER tasks_version_au;
        DROP TRIGGER documents_version_ai;
        DROP TRIGGER documents_version_au;
        DROP TABLE task_versions;
        DROP TABLE document_versions;

        CREATE INDEX versions_task ON versions(task_id, id) WHERE task_id IS NOT NULL;
        CREATE INDEX versions_document ON versions(document_id, id) WHERE document_id IS NOT NULL;
        CREATE INDEX versions_modified ON versions(modified);
        CREATE INDEX versions_workspace ON versions(workspace_id);

        CREATE VIEW version_writes AS
            SELECT NULL AS task_id, NULL AS document_id, NULL AS workspace_id,
                NULL AS old_title, NULL AS old_body, NULL AS old_status,
                NULL AS title, NULL AS body, NULL AS status,
                NULL AS deletion_changed, NULL AS deleted
            WHERE FALSE;

        CREATE TRIGGER version_writes_write INSTEAD OF INSERT ON version_writes
        BEGIN
            INSERT INTO versions (task_id, document_id, workspace_id, title, body, status, change, author)
                SELECT new.task_id, new.document_id, new.workspace_id, new.old_title, new.old_body,
                    new.old_status, 'earlier', 'unknown'
                WHERE NOT EXISTS (SELECT 1 FROM versions
                    WHERE task_id = new.task_id OR document_id = new.document_id);

            INSERT INTO versions (task_id, document_id, workspace_id, title, body, status, change,
                    author, author_name)
                SELECT new.task_id, new.document_id, new.workspace_id, new.title, new.body, new.status,
                    CASE WHEN r.restoring THEN 'restored'
                        WHEN new.deleted THEN 'deleted'
                        ELSE 'edited' END,
                    r.author, r.author_name
                FROM rewind_author r
                WHERE new.deletion_changed OR r.restoring OR NOT EXISTS (
                    SELECT 1 FROM versions v
                    WHERE v.id = (SELECT MAX(id) FROM versions
                            WHERE task_id = new.task_id OR document_id = new.document_id)
                        AND v.change IN ('created', 'edited', 'restored')
                        AND v.created > unixepoch() - 120);

            UPDATE versions
            SET title = new.title, body = new.body, status = new.status,
                change = CASE change WHEN 'restored' THEN 'edited' ELSE change END,
                author = (SELECT author FROM rewind_author),
                author_name = (SELECT author_name FROM rewind_author),
                modified = unixepoch()
            WHERE id = (SELECT MAX(id) FROM versions
                    WHERE task_id = new.task_id OR document_id = new.document_id)
                AND change IN ('created', 'edited', 'restored')
                AND created > unixepoch() - 120
                AND NOT new.deletion_changed
                AND NOT (SELECT restoring FROM rewind_author);
        END;

        CREATE TRIGGER tasks_version_ai AFTER INSERT ON tasks
        BEGIN
            INSERT INTO versions (task_id, workspace_id, title, body, status, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.description, new.status, 'created',
                    author, author_name
                FROM rewind_author;
        END;

        CREATE TRIGGER tasks_version_au
        AFTER UPDATE OF title, description, status, deleted ON tasks
        WHEN old.title IS NOT new.title OR old.description IS NOT new.description
            OR old.status IS NOT new.status OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO version_writes (task_id, workspace_id, old_title, old_body, old_status,
                    title, body, status, deletion_changed, deleted)
                VALUES (new.id, new.workspace_id, old.title, old.description, old.status,
                    new.title, new.description, new.status, new.deleted IS NOT old.deleted,
                    new.deleted AND NOT old.deleted);
        END;

        CREATE TRIGGER documents_version_ai AFTER INSERT ON documents
        BEGIN
            INSERT INTO versions (document_id, workspace_id, title, body, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.contents, 'created', author, author_name
                FROM rewind_author;
        END;

        CREATE TRIGGER documents_version_au
        AFTER UPDATE OF title, contents, deleted ON documents
        WHEN old.title IS NOT new.title OR old.contents IS NOT new.contents
            OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO version_writes (document_id, workspace_id, old_title, old_body,
                    title, body, deletion_changed, deleted)
                VALUES (new.id, new.workspace_id, old.title, old.contents,
                    new.title, new.contents, new.deleted IS NOT old.deleted,
                    new.deleted AND NOT old.deleted);
        END;",
    },
    // Tables that named what they belong to with a kind and an id, or a text
    // key, now name it with real foreign keys: `metadata`, `storage_entries`
    // and `tagged`, which becomes one table for each kind of tag. Their delete
    // triggers go, as do the triggers that held a tag to its item's workspace,
    // which composite foreign keys now hold. A saved view's priorities and
    // sizes leave JSON for rows.
    Migration {
        name: "typed_references",
        sql: "
        CREATE TABLE metadata_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
            document_id INTEGER REFERENCES documents(id) ON DELETE CASCADE,
            task_id INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
            task_folder_id INTEGER REFERENCES task_folders(id) ON DELETE CASCADE,
            resource_id INTEGER REFERENCES resources(id) ON DELETE CASCADE,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            CHECK ((workspace_id IS NOT NULL) + (document_id IS NOT NULL) + (task_id IS NOT NULL)
                + (task_folder_id IS NOT NULL) + (resource_id IS NOT NULL) = 1)
        );
        INSERT INTO metadata_new (id, workspace_id, document_id, task_id, task_folder_id,
                resource_id, key, value)
            SELECT id,
                CASE entity_kind WHEN 'workspaces' THEN entity_id END,
                CASE entity_kind WHEN 'documents' THEN entity_id END,
                CASE entity_kind WHEN 'tasks' THEN entity_id END,
                CASE entity_kind WHEN 'task_folders' THEN entity_id END,
                CASE entity_kind WHEN 'resources' THEN entity_id END,
                key, value
            FROM metadata
            WHERE (entity_kind = 'workspaces' AND entity_id IN (SELECT id FROM workspaces))
                OR (entity_kind = 'documents' AND entity_id IN (SELECT id FROM documents))
                OR (entity_kind = 'tasks' AND entity_id IN (SELECT id FROM tasks))
                OR (entity_kind = 'task_folders' AND entity_id IN (SELECT id FROM task_folders))
                OR (entity_kind = 'resources' AND entity_id IN (SELECT id FROM resources));
        DROP TRIGGER workspaces_metadata_ad;
        DROP TRIGGER documents_metadata_ad;
        DROP TRIGGER tasks_metadata_ad;
        DROP TRIGGER resources_metadata_ad;
        DROP TABLE metadata;
        ALTER TABLE metadata_new RENAME TO metadata;
        CREATE UNIQUE INDEX metadata_workspace ON metadata(workspace_id, key)
            WHERE workspace_id IS NOT NULL;
        CREATE UNIQUE INDEX metadata_document ON metadata(document_id, key)
            WHERE document_id IS NOT NULL;
        CREATE UNIQUE INDEX metadata_task ON metadata(task_id, key) WHERE task_id IS NOT NULL;
        CREATE UNIQUE INDEX metadata_task_folder ON metadata(task_folder_id, key)
            WHERE task_folder_id IS NOT NULL;
        CREATE UNIQUE INDEX metadata_resource ON metadata(resource_id, key)
            WHERE resource_id IS NOT NULL;
        CREATE INDEX metadata_lookup ON metadata(key, value);

        CREATE TABLE storage_entries_new (
            id INTEGER PRIMARY KEY,
            extension_id INTEGER REFERENCES extensions(id) ON DELETE CASCADE,
            agent_id INTEGER REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            CHECK ((extension_id IS NULL) <> (agent_id IS NULL))
        );
        INSERT INTO storage_entries_new (id, extension_id, agent_id, key, value)
            SELECT s.id, e.id, a.id, s.key, s.value
            FROM storage_entries s
            LEFT JOIN extensions e ON s.sandbox = 'extension:' || e.script_identifier
            LEFT JOIN intelligence_agents a ON s.sandbox = 'agent:' || a.id
            WHERE e.id IS NOT NULL OR a.id IS NOT NULL;
        DROP TRIGGER extensions_storage_ad;
        DROP TRIGGER intelligence_agents_storage_ad;
        DROP TABLE storage_entries;
        ALTER TABLE storage_entries_new RENAME TO storage_entries;
        CREATE UNIQUE INDEX storage_entries_extension ON storage_entries(extension_id, key)
            WHERE extension_id IS NOT NULL;
        CREATE UNIQUE INDEX storage_entries_agent ON storage_entries(agent_id, key)
            WHERE agent_id IS NOT NULL;

        CREATE UNIQUE INDEX tags_scope ON tags(workspace_id, entity, id);
        CREATE TABLE task_tags (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            task_id INTEGER NOT NULL,
            tag_entity TEXT NOT NULL DEFAULT 'task' CHECK (tag_entity = 'task'),
            UNIQUE (tag_id, task_id),
            FOREIGN KEY (workspace_id, tag_entity, tag_id)
                REFERENCES tags(workspace_id, entity, id) ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, task_id) REFERENCES tasks(workspace_id, id)
                ON DELETE CASCADE
        );
        CREATE INDEX task_tags_task ON task_tags(task_id);
        CREATE TABLE article_tags (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            article_id INTEGER NOT NULL,
            tag_entity TEXT NOT NULL DEFAULT 'article' CHECK (tag_entity = 'article'),
            UNIQUE (tag_id, article_id),
            FOREIGN KEY (workspace_id, tag_entity, tag_id)
                REFERENCES tags(workspace_id, entity, id) ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, article_id)
                REFERENCES workspace_articles(workspace_id, article_id) ON DELETE CASCADE
        );
        CREATE INDEX article_tags_article ON article_tags(article_id);
        INSERT INTO task_tags (id, workspace_id, tag_id, task_id)
            SELECT x.id, g.workspace_id, x.tag_id, x.entity_id
            FROM tagged x
            JOIN tags g ON g.id = x.tag_id AND g.entity = 'task'
            JOIN tasks t ON t.id = x.entity_id AND t.workspace_id = g.workspace_id;
        INSERT INTO article_tags (id, workspace_id, tag_id, article_id)
            SELECT x.id, g.workspace_id, x.tag_id, x.entity_id
            FROM tagged x
            JOIN tags g ON g.id = x.tag_id AND g.entity = 'article'
            JOIN workspace_articles w ON w.article_id = x.entity_id
                AND w.workspace_id = g.workspace_id;
        DROP TRIGGER tasks_tagged_ad;
        DROP TRIGGER workspace_articles_tagged_ad;
        DROP TABLE tagged;

        CREATE UNIQUE INDEX saved_task_views_scope ON saved_task_views(workspace_id, id);
        CREATE TABLE saved_task_view_tags_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL,
            view_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            tag_match TEXT NOT NULL,
            tag_entity TEXT NOT NULL DEFAULT 'task' CHECK (tag_entity = 'task'),
            UNIQUE (view_id, tag_id),
            FOREIGN KEY (workspace_id, view_id) REFERENCES saved_task_views(workspace_id, id)
                ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, tag_entity, tag_id)
                REFERENCES tags(workspace_id, entity, id) ON DELETE CASCADE
        );
        INSERT INTO saved_task_view_tags_new (id, workspace_id, view_id, tag_id, tag_match)
            SELECT x.id, v.workspace_id, x.view_id, x.tag_id, x.tag_match
            FROM saved_task_view_tags x
            JOIN saved_task_views v ON v.id = x.view_id
            JOIN tags g ON g.id = x.tag_id AND g.entity = 'task'
                AND g.workspace_id = v.workspace_id;
        DROP TABLE saved_task_view_tags;
        ALTER TABLE saved_task_view_tags_new RENAME TO saved_task_view_tags;

        CREATE TABLE saved_task_view_priorities (
            id INTEGER PRIMARY KEY,
            view_id INTEGER NOT NULL REFERENCES saved_task_views(id) ON DELETE CASCADE,
            priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 4),
            UNIQUE (view_id, priority)
        );
        INSERT OR IGNORE INTO saved_task_view_priorities (view_id, priority)
            SELECT v.id, CASE p.value
                    WHEN 'none' THEN 0 WHEN 'low' THEN 1 WHEN 'medium' THEN 2
                    WHEN 'high' THEN 3 WHEN 'urgent' THEN 4 END
            FROM saved_task_views v, json_each(v.priorities) p;
        CREATE TABLE saved_task_view_sizes (
            id INTEGER PRIMARY KEY,
            view_id INTEGER NOT NULL REFERENCES saved_task_views(id) ON DELETE CASCADE,
            size INTEGER NOT NULL CHECK (size BETWEEN 1 AND 5),
            UNIQUE (view_id, size)
        );
        INSERT OR IGNORE INTO saved_task_view_sizes (view_id, size)
            SELECT v.id, s.value FROM saved_task_views v, json_each(v.sizes) s;
        ALTER TABLE saved_task_views DROP COLUMN priorities;
        ALTER TABLE saved_task_views DROP COLUMN sizes;",
    },
    // What's left of the states a row's columns only spell by convention:
    // a task's folder is held to its workspace, tasks and documents say when
    // they were deleted so the cleanup can end them, a run's outcome carries
    // what only that outcome has, an article's snapshot and a feed's status
    // leave JSON for columns, and the new retentions start. Foreign keys that
    // a delete has to scan for get their indexes.
    Migration {
        name: "lifecycles_and_states",
        sql: "
        CREATE UNIQUE INDEX task_folders_scope ON task_folders(workspace_id, id);
        CREATE TABLE tasks_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
            task_folder_id INTEGER,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'incomplete',
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            priority INTEGER NOT NULL DEFAULT 0,
            description TEXT NOT NULL DEFAULT '',
            start_date TEXT CHECK (start_date IS date(start_date)),
            due_date TEXT CHECK (due_date IS date(due_date)),
            size INTEGER CHECK (size BETWEEN 1 AND 5),
            assigned_to_user BOOLEAN NOT NULL DEFAULT FALSE,
            assignee_agent_id INTEGER
                REFERENCES intelligence_agents(id) ON DELETE SET NULL
                CHECK (NOT (assigned_to_user AND assignee_agent_id IS NOT NULL)),
            assignee_contact_id INTEGER
                REFERENCES contacts(id) ON DELETE SET NULL
                CHECK (assignee_contact_id IS NULL
                    OR (NOT assigned_to_user AND assignee_agent_id IS NULL)),
            deleted_at INTEGER,
            FOREIGN KEY (workspace_id, task_folder_id) REFERENCES task_folders(workspace_id, id)
                ON DELETE CASCADE
        );
        INSERT INTO tasks_new (id, workspace_id, task_folder_id, title, status, deleted, priority,
                description, start_date, due_date, size, assigned_to_user, assignee_agent_id,
                assignee_contact_id, deleted_at)
            SELECT id, workspace_id, task_folder_id, title, status, deleted, priority, description,
                start_date, due_date, size, assigned_to_user, assignee_agent_id,
                assignee_contact_id, CASE WHEN deleted THEN unixepoch() END
            FROM tasks;
        PRAGMA legacy_alter_table = ON;
        DROP TABLE tasks;
        ALTER TABLE tasks_new RENAME TO tasks;
        PRAGMA legacy_alter_table = OFF;
        CREATE UNIQUE INDEX tasks_workspace_id ON tasks(workspace_id, id);
        CREATE INDEX tasks_folder ON tasks(task_folder_id) WHERE task_folder_id IS NOT NULL;
        CREATE INDEX tasks_assignee_agent ON tasks(assignee_agent_id);
        CREATE INDEX tasks_assignee_contact ON tasks(assignee_contact_id);
        CREATE INDEX tasks_deleted_at ON tasks(deleted_at) WHERE deleted_at IS NOT NULL;
        CREATE TRIGGER tasks_search_ai AFTER INSERT ON tasks WHEN new.deleted = 0 BEGIN
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                VALUES ('task', new.id, new.workspace_id);
            INSERT INTO search_fts (rowid, title, contents)
                VALUES (last_insert_rowid(), new.title, new.description);
        END;
        CREATE TRIGGER tasks_search_ad AFTER DELETE ON tasks BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id;
        END;
        CREATE TRIGGER tasks_search_au
        AFTER UPDATE OF title, description, deleted, workspace_id ON tasks BEGIN
            DELETE FROM search_fts WHERE rowid =
                (SELECT id FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'task' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                SELECT 'task', new.id, new.workspace_id WHERE new.deleted = 0;
            INSERT INTO search_fts (rowid, title, contents)
                SELECT last_insert_rowid(), new.title, new.description WHERE new.deleted = 0;
        END;
        CREATE TRIGGER tasks_assignee_contact_bi BEFORE INSERT ON tasks
        WHEN new.assignee_contact_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM contacts WHERE contacts.id = new.assignee_contact_id
                AND contacts.workspace_id = new.workspace_id AND NOT contacts.deleted
        )
        BEGIN
            SELECT RAISE(ABORT, 'A task is assigned only to a live contact of its workspace.');
        END;
        CREATE TRIGGER tasks_assignee_contact_bu
        BEFORE UPDATE OF assignee_contact_id, workspace_id ON tasks
        WHEN new.assignee_contact_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM contacts WHERE contacts.id = new.assignee_contact_id
                AND contacts.workspace_id = new.workspace_id AND NOT contacts.deleted
        )
        BEGIN
            SELECT RAISE(ABORT, 'A task is assigned only to a live contact of its workspace.');
        END;
        CREATE TRIGGER tasks_version_ai AFTER INSERT ON tasks
        BEGIN
            INSERT INTO versions (task_id, workspace_id, title, body, status, change, author, author_name)
                SELECT new.id, new.workspace_id, new.title, new.description, new.status, 'created',
                    author, author_name
                FROM rewind_author;
        END;

        CREATE TRIGGER tasks_version_au
        AFTER UPDATE OF title, description, status, deleted ON tasks
        WHEN old.title IS NOT new.title OR old.description IS NOT new.description
            OR old.status IS NOT new.status OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO version_writes (task_id, workspace_id, old_title, old_body, old_status,
                    title, body, status, deletion_changed, deleted)
                VALUES (new.id, new.workspace_id, old.title, old.description, old.status,
                    new.title, new.description, new.status, new.deleted IS NOT old.deleted,
                    new.deleted AND NOT old.deleted);
        END;
        CREATE TRIGGER tasks_deleted_at AFTER UPDATE OF deleted ON tasks
        WHEN new.deleted IS NOT old.deleted
        BEGIN
            UPDATE tasks SET deleted_at = CASE WHEN new.deleted THEN unixepoch() END
            WHERE id = new.id;
        END;

        ALTER TABLE documents ADD COLUMN deleted_at INTEGER;
        UPDATE documents SET deleted_at = unixepoch() WHERE deleted;
        CREATE INDEX documents_deleted_at ON documents(deleted_at) WHERE deleted_at IS NOT NULL;
        CREATE TRIGGER documents_deleted_at AFTER UPDATE OF deleted ON documents
        WHEN new.deleted IS NOT old.deleted
        BEGIN
            UPDATE documents SET deleted_at = CASE WHEN new.deleted THEN unixepoch() END
            WHERE id = new.id;
        END;

        CREATE TABLE intelligence_agent_runs_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            thread_id INTEGER REFERENCES intelligence_threads(id) ON DELETE SET NULL,
            hook TEXT NOT NULL,
            started_at INTEGER NOT NULL DEFAULT (unixepoch()),
            outcome TEXT NOT NULL DEFAULT 'running'
                CHECK (outcome IN ('running', 'completed', 'failed')),
            ended_at INTEGER,
            error TEXT,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cost REAL,
            cue_name TEXT,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0,
            sender_agent_id INTEGER REFERENCES intelligence_agents(id) ON DELETE SET NULL,
            CHECK ((ended_at IS NULL) = (outcome = 'running')),
            CHECK ((error IS NOT NULL) = (outcome = 'failed'))
        );
        INSERT INTO intelligence_agent_runs_new (id, agent_id, thread_id, hook, started_at, outcome,
                ended_at, error, input_tokens, output_tokens, cost, cue_name, cache_read_tokens,
                cache_write_tokens, sender_agent_id)
            SELECT id, agent_id, thread_id, hook, started_at, outcome,
                CASE WHEN outcome = 'running' THEN NULL ELSE coalesce(ended_at, started_at) END,
                CASE WHEN outcome = 'failed' THEN coalesce(error, '') END,
                input_tokens, output_tokens, cost, cue_name, cache_read_tokens, cache_write_tokens,
                sender_agent_id
            FROM intelligence_agent_runs;
        PRAGMA legacy_alter_table = ON;
        DROP TABLE intelligence_agent_runs;
        ALTER TABLE intelligence_agent_runs_new RENAME TO intelligence_agent_runs;
        PRAGMA legacy_alter_table = OFF;
        CREATE INDEX intelligence_agent_runs_agent ON intelligence_agent_runs(agent_id, id);
        CREATE INDEX intelligence_agent_runs_thread ON intelligence_agent_runs(thread_id)
            WHERE thread_id IS NOT NULL;
        CREATE INDEX intelligence_agent_runs_sender ON intelligence_agent_runs(sender_agent_id)
            WHERE sender_agent_id IS NOT NULL;

        CREATE TABLE articles_new (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            feed_id INTEGER REFERENCES feeds(id) ON DELETE SET NULL,
            read BOOLEAN NOT NULL DEFAULT FALSE,
            url TEXT NOT NULL,
            title TEXT NOT NULL,
            author TEXT,
            published_at INTEGER NOT NULL DEFAULT 0,
            contents TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            published_precision TEXT NOT NULL DEFAULT 'time',
            snapshot_state TEXT NOT NULL CHECK (snapshot_state IN ('captured', 'failed')),
            snapshot_at INTEGER NOT NULL,
            final_url TEXT,
            page_title TEXT,
            truncated BOOLEAN,
            failure_kind TEXT,
            failure_text TEXT,
            failure_code INTEGER,
            CHECK (contents = '' OR snapshot_state = 'captured'),
            CHECK ((final_url IS NOT NULL) = (snapshot_state = 'captured')),
            CHECK ((truncated IS NOT NULL) = (snapshot_state = 'captured')),
            CHECK ((failure_kind IS NOT NULL) = (snapshot_state = 'failed'))
        );
        INSERT INTO articles_new (id, deleted, feed_id, read, url, title, author, published_at,
                contents, created_at, modified, published_precision, snapshot_state, snapshot_at,
                final_url, page_title, truncated, failure_kind, failure_text, failure_code)
            SELECT id, deleted, feed_id, read, url, title, author, published_at, contents,
                created_at, modified, published_precision,
                snapshot ->> '$.state',
                coalesce(snapshot ->> '$.fetched_at', snapshot ->> '$.attempted_at', created_at),
                snapshot ->> '$.final_url',
                snapshot ->> '$.page_title',
                snapshot ->> '$.truncated',
                snapshot ->> '$.failure.kind',
                coalesce(snapshot ->> '$.failure.message', snapshot ->> '$.failure.content_type'),
                snapshot ->> '$.failure.code'
            FROM articles;
        PRAGMA legacy_alter_table = ON;
        DROP TABLE articles;
        ALTER TABLE articles_new RENAME TO articles;
        PRAGMA legacy_alter_table = OFF;
        CREATE INDEX articles_feed_url ON articles(feed_id, url) WHERE feed_id IS NOT NULL;
        CREATE INDEX articles_url ON articles(url) WHERE deleted = FALSE;
        CREATE TRIGGER articles_feed_corpus_ai AFTER INSERT ON articles
        WHEN new.feed_id IS NOT NULL AND new.deleted = 0 BEGIN
            INSERT INTO feed_corpus_fts (rowid, title, author, contents)
                VALUES (new.id, new.title, new.author, new.contents);
        END;
        CREATE TRIGGER articles_feed_corpus_ad AFTER DELETE ON articles BEGIN
            DELETE FROM feed_corpus_fts WHERE rowid = old.id;
        END;
        CREATE TRIGGER articles_feed_corpus_au
        AFTER UPDATE OF title, author, contents, deleted, feed_id ON articles BEGIN
            DELETE FROM feed_corpus_fts WHERE rowid = old.id;
            INSERT INTO feed_corpus_fts (rowid, title, author, contents)
                SELECT new.id, new.title, new.author, new.contents
                WHERE new.feed_id IS NOT NULL AND new.deleted = 0;
        END;
        CREATE TRIGGER articles_search_ad AFTER DELETE ON articles BEGIN
            DELETE FROM search_fts WHERE rowid IN
                (SELECT id FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id;
        END;
        CREATE TRIGGER articles_search_au AFTER UPDATE OF title, contents, deleted ON articles BEGIN
            DELETE FROM search_fts WHERE rowid IN
                (SELECT id FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id);
            DELETE FROM search_entries WHERE entity_kind = 'article' AND entity_id = old.id;
            INSERT INTO search_entries (entity_kind, entity_id, workspace_id)
                SELECT 'article', new.id, workspace_id FROM workspace_articles
                WHERE article_id = new.id AND new.deleted = 0;
            INSERT INTO search_fts (rowid, title, contents)
                SELECT id, new.title, new.contents FROM search_entries
                WHERE entity_kind = 'article' AND entity_id = new.id;
        END;

        CREATE TABLE feeds_new (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            url TEXT NOT NULL,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            full_page_policy TEXT NOT NULL DEFAULT 'whenExcerpt',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            parser TEXT NOT NULL DEFAULT 'coherence',
            status_state TEXT NOT NULL CHECK (status_state IN ('refreshed', 'failed')),
            status_at INTEGER NOT NULL,
            added INTEGER,
            failure_kind TEXT,
            failure_text TEXT,
            failure_code INTEGER,
            CHECK ((added IS NOT NULL) = (status_state = 'refreshed')),
            CHECK ((failure_kind IS NOT NULL) = (status_state = 'failed'))
        );
        INSERT INTO feeds_new (id, deleted, url, title, sort_order, full_page_policy, created_at,
                parser, status_state, status_at, added, failure_kind, failure_text, failure_code)
            SELECT id, deleted, url, title, sort_order, full_page_policy, created_at, parser,
                status ->> '$.state', coalesce(status ->> '$.at', created_at),
                status ->> '$.added',
                status ->> '$.failure.kind',
                coalesce(status ->> '$.failure.message', status ->> '$.failure.content_type'),
                status ->> '$.failure.code'
            FROM feeds;
        PRAGMA legacy_alter_table = ON;
        DROP TABLE feeds;
        ALTER TABLE feeds_new RENAME TO feeds;
        PRAGMA legacy_alter_table = OFF;
        CREATE UNIQUE INDEX feeds_url ON feeds(url) WHERE deleted = FALSE;

        ALTER TABLE storage_settings ADD COLUMN fork_retention_days INTEGER NOT NULL DEFAULT 3
            CHECK (fork_retention_days >= 1);
        ALTER TABLE storage_settings ADD COLUMN attachment_retention_days INTEGER NOT NULL DEFAULT 3
            CHECK (attachment_retention_days >= 1);
        ALTER TABLE storage_settings ADD COLUMN code_result_retention_days INTEGER NOT NULL DEFAULT 30
            CHECK (code_result_retention_days >= 1);
        ALTER TABLE storage_settings ADD COLUMN deleted_item_retention_days INTEGER NOT NULL DEFAULT 30
            CHECK (deleted_item_retention_days >= 1);

        CREATE INDEX actor_tasks_actor ON actor_tasks(actor_id);
        CREATE INDEX actor_tasks_follower ON actor_tasks(follower_agent_id)
            WHERE follower_agent_id IS NOT NULL;
        CREATE INDEX connector_accounts_connector ON connector_accounts(connector_id);
        CREATE INDEX article_folders_workspace ON article_folders(workspace_id);
        CREATE INDEX workspace_articles_folder ON workspace_articles(article_folder_id)
            WHERE article_folder_id IS NOT NULL;
        CREATE INDEX security_events_client ON security_events(client_id)
            WHERE client_id IS NOT NULL;",
    },
    // Images follow their actor task, script results and forks follow the model
    // thread retention, and security activity keeps a fixed 30 days.
    Migration {
        name: "fewer_storage_settings",
        sql: "ALTER TABLE storage_settings DROP COLUMN security_event_retention_days;
        ALTER TABLE storage_settings DROP COLUMN fork_retention_days;
        ALTER TABLE storage_settings DROP COLUMN attachment_retention_days;
        ALTER TABLE storage_settings DROP COLUMN code_result_retention_days;",
    },
    // An agent has one marked thread, its main one: a cue's run starts fresh
    // now, so no fork shares an opening of its own.
    Migration {
        name: "one_thread_mark_per_agent",
        sql: "CREATE TABLE agent_thread_marks_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL UNIQUE REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            thread_id INTEGER NOT NULL REFERENCES intelligence_threads(id) ON DELETE CASCADE,
            items INTEGER NOT NULL CHECK (items >= 0),
            endpoint_model_id INTEGER NOT NULL REFERENCES endpoint_models(id) ON DELETE CASCADE,
            heard_through INTEGER NOT NULL,
            memories TEXT NOT NULL,
            last_call_at INTEGER NOT NULL,
            prompt_tokens INTEGER NOT NULL CHECK (prompt_tokens >= 0),
            cache_seconds INTEGER NOT NULL CHECK (cache_seconds > 0),
            extended_from INTEGER NOT NULL
        );
        INSERT INTO agent_thread_marks_new (id, agent_id, thread_id, items, endpoint_model_id,
                heard_through, memories, last_call_at, prompt_tokens, cache_seconds, extended_from)
            SELECT id, agent_id, thread_id, items, endpoint_model_id, heard_through, memories,
                last_call_at, prompt_tokens, cache_seconds, extended_from
            FROM agent_thread_marks WHERE source = 'conversation';
        DROP TABLE agent_thread_marks;
        ALTER TABLE agent_thread_marks_new RENAME TO agent_thread_marks;
        CREATE INDEX agent_thread_marks_thread ON agent_thread_marks(thread_id);
        CREATE INDEX agent_thread_marks_endpoint_model
            ON agent_thread_marks(endpoint_model_id);",
    },
    // An actor task is an actor session, in the schema as everywhere else, so
    // every row stays and only its names move. Triggers go first, since a rename
    // can't run past one naming what it renames, and come back under the new
    // names; indexes aren't renamed by a rename, so they are made again too. The
    // agent messages' `wake` word is held by a `CHECK`, so that table is rebuilt.
    // Stored links move to the new scheme, the only one the clients read, without
    // Rewind counting that as an edit: its triggers on the two tables with history
    // are out for the rewrite. A live model thread's stored prompt declares the
    // old script API and would keep calling it, so no thread carries on: marks go,
    // and each agent's next turn starts a new thread, as after any lost mark.
    Migration {
        name: "actor_tasks_become_actor_sessions",
        sql: "DROP TRIGGER actor_tasks_search_ai;
        DROP TRIGGER actor_tasks_search_ad;
        DROP TRIGGER actor_tasks_search_au;
        DROP TRIGGER actor_tasks_status_changed;
        DROP TRIGGER actor_tasks_valid_insert;
        DROP TRIGGER actor_tasks_valid_update;
        DROP TRIGGER activities_actor_task_unread;
        DROP TRIGGER actor_task_messages_search_ai;
        DROP TRIGGER actor_task_messages_search_ad;
        DROP TRIGGER actor_task_messages_search_au;
        DROP INDEX actor_tasks_recent;
        DROP INDEX actor_tasks_actor;
        DROP INDEX actor_tasks_follower;
        DROP INDEX actor_task_messages_task;
        DROP INDEX actor_task_messages_one_response;
        DROP INDEX activities_one_open_actor_task;
        DROP INDEX attachments_actor_task_message;
        ALTER TABLE actor_tasks RENAME TO actor_sessions;
        ALTER TABLE actor_tasks_fts RENAME TO actor_sessions_fts;
        ALTER TABLE actor_task_messages RENAME TO actor_session_messages;
        ALTER TABLE actor_task_messages_fts RENAME TO actor_session_messages_fts;
        ALTER TABLE actor_session_messages RENAME COLUMN task_id TO session_id;
        ALTER TABLE activities RENAME COLUMN actor_task_id TO actor_session_id;
        ALTER TABLE attachments RENAME COLUMN actor_task_message_id TO actor_session_message_id;
        ALTER TABLE storage_settings
            RENAME COLUMN actor_task_retention_days TO actor_session_retention_days;
        CREATE INDEX actor_sessions_recent ON actor_sessions(status_changed_at);
        CREATE INDEX actor_sessions_actor ON actor_sessions(actor_id);
        CREATE INDEX actor_sessions_follower ON actor_sessions(follower_agent_id)
            WHERE follower_agent_id IS NOT NULL;
        CREATE INDEX actor_session_messages_session ON actor_session_messages(session_id, id);
        CREATE UNIQUE INDEX actor_session_messages_one_response
            ON actor_session_messages(request_id) WHERE request_id IS NOT NULL;
        CREATE UNIQUE INDEX activities_one_open_actor_session ON activities(actor_session_id)
            WHERE ended_at IS NULL;
        CREATE INDEX attachments_actor_session_message ON attachments(actor_session_message_id)
            WHERE actor_session_message_id IS NOT NULL;
        CREATE TRIGGER actor_sessions_search_ai AFTER INSERT ON actor_sessions
        BEGIN
            INSERT INTO actor_sessions_fts (rowid, title) VALUES (new.id, new.title);
        END;
        CREATE TRIGGER actor_sessions_search_ad AFTER DELETE ON actor_sessions
        BEGIN
            DELETE FROM actor_sessions_fts WHERE rowid = old.id;
        END;
        CREATE TRIGGER actor_sessions_search_au AFTER UPDATE OF title ON actor_sessions
        BEGIN
            DELETE FROM actor_sessions_fts WHERE rowid = old.id;
            INSERT INTO actor_sessions_fts (rowid, title) VALUES (new.id, new.title);
        END;
        CREATE TRIGGER actor_sessions_status_changed AFTER UPDATE OF status ON actor_sessions
        WHEN new.status <> old.status
        BEGIN
            UPDATE actor_sessions SET status_changed_at = unixepoch() WHERE id = new.id;
        END;
        CREATE TRIGGER actor_sessions_valid_insert BEFORE INSERT ON actor_sessions
        WHEN new.status NOT IN ('working', 'needsAttention', 'idle', 'failed', 'resolved')
        BEGIN
            SELECT RAISE(ABORT, 'An actor session is working, needs attention, idle, failed or resolved.');
        END;
        CREATE TRIGGER actor_sessions_valid_update BEFORE UPDATE OF status ON actor_sessions
        WHEN new.status NOT IN ('working', 'needsAttention', 'idle', 'failed', 'resolved')
        BEGIN
            SELECT RAISE(ABORT, 'An actor session is working, needs attention, idle, failed or resolved.');
        END;
        CREATE TRIGGER activities_actor_session_unread AFTER UPDATE OF status ON activities
        WHEN new.status IN ('succeeded', 'failed')
        BEGIN
            UPDATE actor_sessions SET unread = TRUE WHERE id = new.actor_session_id;
        END;
        CREATE TRIGGER actor_session_messages_search_ai AFTER INSERT ON actor_session_messages
        WHEN new.kind = 'text'
        BEGIN
            INSERT INTO actor_session_messages_fts (rowid, text) VALUES (new.id, new.text);
        END;
        CREATE TRIGGER actor_session_messages_search_ad AFTER DELETE ON actor_session_messages
        BEGIN
            DELETE FROM actor_session_messages_fts WHERE rowid = old.id;
        END;
        CREATE TRIGGER actor_session_messages_search_au AFTER UPDATE OF text ON actor_session_messages
        BEGIN
            DELETE FROM actor_session_messages_fts WHERE rowid = old.id;
            INSERT INTO actor_session_messages_fts (rowid, text)
                SELECT new.id, new.text WHERE new.kind = 'text';
        END;
        CREATE TABLE intelligence_agent_messages_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            run_id INTEGER REFERENCES intelligence_agent_runs(id) ON DELETE SET NULL,
            run_description TEXT,
            sent_at INTEGER NOT NULL DEFAULT (unixepoch()),
            sender TEXT NOT NULL,
            reaction TEXT,
            reacted_at INTEGER,
            reply_to INTEGER REFERENCES intelligence_agent_messages(id) ON DELETE SET NULL,
            kind TEXT NOT NULL CHECK (kind IN ('text', 'codeExecution', 'error', 'documentation',
                'choice', 'memory', 'forward', 'askedAgent', 'askedByAgent', 'note', 'woke',
                'compressedContext')),
            text TEXT,
            title TEXT,
            code TEXT,
            outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'cancelled')),
            outcome_text TEXT,
            options TEXT,
            context TEXT,
            selected INTEGER,
            change TEXT CHECK (change IN ('remembered', 'forgotten')),
            item_kind TEXT,
            item_workspace_id INTEGER,
            item_id INTEGER,
            peer_id INTEGER,
            peer_name TEXT,
            reply TEXT CHECK (reply IN ('waiting', 'answered', 'unanswered')),
            reply_text TEXT,
            wake TEXT CHECK (wake IN ('cue', 'actorSession', 'assignedTask')),
            cue_name TEXT,
            hook TEXT,
            actor_session_id INTEGER,
            stop TEXT,
            reason TEXT,
            CHECK ((text IS NOT NULL) = (kind NOT IN ('codeExecution', 'woke', 'compressedContext'))),
            CHECK ((outcome IS NOT NULL) = (kind = 'codeExecution')),
            CHECK ((code IS NOT NULL) = (kind = 'codeExecution')),
            CHECK ((options IS NOT NULL) = (kind = 'choice')),
            CHECK ((change IS NOT NULL) = (kind = 'memory')),
            CHECK ((item_kind IS NOT NULL) = (kind = 'forward')),
            CHECK ((peer_id IS NOT NULL) = (kind IN ('askedAgent', 'askedByAgent'))),
            CHECK ((reply IS NOT NULL) = (kind IN ('askedAgent', 'askedByAgent'))),
            CHECK ((wake IS NOT NULL) = (kind = 'woke'))
        );
        INSERT INTO intelligence_agent_messages_new (id, agent_id, run_id, run_description, sent_at,
                sender, reaction, reacted_at, reply_to, kind, text, title, code, outcome,
                outcome_text, options, context, selected, change, item_kind, item_workspace_id,
                item_id, peer_id, peer_name, reply, reply_text, wake, cue_name, hook,
                actor_session_id, stop, reason)
            SELECT id, agent_id, run_id, run_description, sent_at, sender, reaction, reacted_at,
                reply_to, kind, text, title, code, outcome, outcome_text, options, context,
                selected, change, item_kind, item_workspace_id, item_id, peer_id, peer_name, reply,
                reply_text, CASE wake WHEN 'actorTask' THEN 'actorSession' ELSE wake END, cue_name,
                hook, actor_task_id, stop, reason
            FROM intelligence_agent_messages;
        DROP TABLE intelligence_agent_messages;
        ALTER TABLE intelligence_agent_messages_new RENAME TO intelligence_agent_messages;
        CREATE INDEX intelligence_agent_messages_agent
            ON intelligence_agent_messages(agent_id, id);
        CREATE INDEX intelligence_agent_messages_run ON intelligence_agent_messages(run_id)
            WHERE run_id IS NOT NULL;
        CREATE INDEX intelligence_agent_messages_reply_to ON intelligence_agent_messages(reply_to)
            WHERE reply_to IS NOT NULL;
        CREATE TRIGGER intelligence_agent_messages_reply_to
        BEFORE INSERT ON intelligence_agent_messages
        WHEN new.reply_to IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM intelligence_agent_messages
            WHERE id = new.reply_to AND agent_id = new.agent_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'A reply answers a message of the same conversation.');
        END;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN new.kind NOT IN ('codeExecution', 'documentation', 'memory', 'askedAgent',
            'askedByAgent', 'note', 'woke', 'compressedContext')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;
        DROP TRIGGER documents_version_au;
        DROP TRIGGER tasks_version_au;
        UPDATE documents SET contents = replace(contents, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(contents, 'cohesion://actor-task/') > 0;
        UPDATE tasks SET description = replace(description, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(description, 'cohesion://actor-task/') > 0;
        UPDATE versions SET body = replace(body, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(body, 'cohesion://actor-task/') > 0;
        UPDATE memories SET memory = replace(memory, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(memory, 'cohesion://actor-task/') > 0;
        UPDATE metadata SET value = replace(value, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(value, 'cohesion://actor-task/') > 0;
        UPDATE storage_entries SET value = replace(value, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(value, 'cohesion://actor-task/') > 0;
        UPDATE actor_session_messages SET text = replace(text, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(text, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_agent_messages SET text = replace(text, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(text, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_agent_messages SET code = replace(code, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(code, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_agent_messages SET outcome_text = replace(outcome_text, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(outcome_text, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_agent_messages SET context = replace(context, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(context, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_agent_messages SET reply_text = replace(reply_text, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(reply_text, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_thread_messages SET text = replace(text, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(text, 'cohesion://actor-task/') > 0;
        UPDATE intelligence_thread_messages SET arguments = replace(arguments, 'cohesion://actor-task/', 'cohesion://actor-session/')
            WHERE instr(arguments, 'cohesion://actor-task/') > 0;
        CREATE TRIGGER documents_version_au
        AFTER UPDATE OF title, contents, deleted ON documents
        WHEN old.title IS NOT new.title OR old.contents IS NOT new.contents
            OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO version_writes (document_id, workspace_id, old_title, old_body,
                    title, body, deletion_changed, deleted)
                VALUES (new.id, new.workspace_id, old.title, old.contents,
                    new.title, new.contents, new.deleted IS NOT old.deleted,
                    new.deleted AND NOT old.deleted);
        END;
        CREATE TRIGGER tasks_version_au
        AFTER UPDATE OF title, description, status, deleted ON tasks
        WHEN old.title IS NOT new.title OR old.description IS NOT new.description
            OR old.status IS NOT new.status OR old.deleted IS NOT new.deleted
        BEGIN
            INSERT INTO version_writes (task_id, workspace_id, old_title, old_body, old_status,
                    title, body, status, deletion_changed, deleted)
                VALUES (new.id, new.workspace_id, old.title, old.description, old.status,
                    new.title, new.description, new.status, new.deleted IS NOT old.deleted,
                    new.deleted AND NOT old.deleted);
        END;
        DELETE FROM agent_thread_marks;
        UPDATE intelligence_agents SET context_tokens = NULL;",
    },
    // Coherence is the only article parser, so a feed has none to choose.
    Migration {
        name: "feeds_drop_parser",
        sql: "ALTER TABLE feeds DROP COLUMN parser;",
    },
    // A thread mark says how many memories its thread's system prompt lets each
    // scope keep, so a changed setting can rebuild the prompt. The prompts
    // already stored are taken to state the setting as it is now, so this
    // rebuilds no thread.
    Migration {
        name: "agent_thread_mark_memory_limit",
        sql: "CREATE TABLE agent_thread_marks_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL UNIQUE REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            thread_id INTEGER NOT NULL REFERENCES intelligence_threads(id) ON DELETE CASCADE,
            items INTEGER NOT NULL CHECK (items >= 0),
            endpoint_model_id INTEGER NOT NULL REFERENCES endpoint_models(id) ON DELETE CASCADE,
            heard_through INTEGER NOT NULL,
            memories TEXT NOT NULL,
            last_call_at INTEGER NOT NULL,
            prompt_tokens INTEGER NOT NULL CHECK (prompt_tokens >= 0),
            cache_seconds INTEGER NOT NULL CHECK (cache_seconds > 0),
            extended_from INTEGER NOT NULL,
            memory_limit INTEGER NOT NULL CHECK (memory_limit >= 1)
        );
        INSERT INTO agent_thread_marks_new (id, agent_id, thread_id, items, endpoint_model_id,
                heard_through, memories, last_call_at, prompt_tokens, cache_seconds, extended_from,
                memory_limit)
            SELECT id, agent_id, thread_id, items, endpoint_model_id, heard_through, memories,
                last_call_at, prompt_tokens, cache_seconds, extended_from,
                (SELECT memory_limit FROM storage_settings)
            FROM agent_thread_marks;
        DROP TABLE agent_thread_marks;
        ALTER TABLE agent_thread_marks_new RENAME TO agent_thread_marks;
        CREATE INDEX agent_thread_marks_thread ON agent_thread_marks(thread_id);
        CREATE INDEX agent_thread_marks_endpoint_model
            ON agent_thread_marks(endpoint_model_id);",
    },
    // A thread that can't carry on is rebuilt, whatever the reason, so its line
    // says so rather than `compressedContext`.
    Migration {
        name: "agent_message_rebuilt_context",
        sql: "CREATE TABLE intelligence_agent_messages_new (
            id INTEGER PRIMARY KEY,
            agent_id INTEGER NOT NULL REFERENCES intelligence_agents(id) ON DELETE CASCADE,
            run_id INTEGER REFERENCES intelligence_agent_runs(id) ON DELETE SET NULL,
            run_description TEXT,
            sent_at INTEGER NOT NULL DEFAULT (unixepoch()),
            sender TEXT NOT NULL,
            reaction TEXT,
            reacted_at INTEGER,
            reply_to INTEGER REFERENCES intelligence_agent_messages(id) ON DELETE SET NULL,
            kind TEXT NOT NULL CHECK (kind IN ('text', 'codeExecution', 'error', 'documentation',
                'choice', 'memory', 'forward', 'askedAgent', 'askedByAgent', 'note', 'woke',
                'rebuiltContext')),
            text TEXT,
            title TEXT,
            code TEXT,
            outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'cancelled')),
            outcome_text TEXT,
            options TEXT,
            context TEXT,
            selected INTEGER,
            change TEXT CHECK (change IN ('remembered', 'forgotten')),
            item_kind TEXT,
            item_workspace_id INTEGER,
            item_id INTEGER,
            peer_id INTEGER,
            peer_name TEXT,
            reply TEXT CHECK (reply IN ('waiting', 'answered', 'unanswered')),
            reply_text TEXT,
            wake TEXT CHECK (wake IN ('cue', 'actorSession', 'assignedTask')),
            cue_name TEXT,
            hook TEXT,
            actor_session_id INTEGER,
            stop TEXT,
            reason TEXT,
            CHECK ((text IS NOT NULL) = (kind NOT IN ('codeExecution', 'woke', 'rebuiltContext'))),
            CHECK ((outcome IS NOT NULL) = (kind = 'codeExecution')),
            CHECK ((code IS NOT NULL) = (kind = 'codeExecution')),
            CHECK ((options IS NOT NULL) = (kind = 'choice')),
            CHECK ((change IS NOT NULL) = (kind = 'memory')),
            CHECK ((item_kind IS NOT NULL) = (kind = 'forward')),
            CHECK ((peer_id IS NOT NULL) = (kind IN ('askedAgent', 'askedByAgent'))),
            CHECK ((reply IS NOT NULL) = (kind IN ('askedAgent', 'askedByAgent'))),
            CHECK ((wake IS NOT NULL) = (kind = 'woke'))
        );
        INSERT INTO intelligence_agent_messages_new (id, agent_id, run_id, run_description, sent_at,
                sender, reaction, reacted_at, reply_to, kind, text, title, code, outcome,
                outcome_text, options, context, selected, change, item_kind, item_workspace_id,
                item_id, peer_id, peer_name, reply, reply_text, wake, cue_name, hook,
                actor_session_id, stop, reason)
            SELECT id, agent_id, run_id, run_description, sent_at, sender, reaction, reacted_at,
                reply_to, CASE kind WHEN 'compressedContext' THEN 'rebuiltContext' ELSE kind END,
                text, title, code, outcome, outcome_text, options, context, selected, change,
                item_kind, item_workspace_id, item_id, peer_id, peer_name, reply, reply_text, wake,
                cue_name, hook, actor_session_id, stop, reason
            FROM intelligence_agent_messages;
        DROP TABLE intelligence_agent_messages;
        ALTER TABLE intelligence_agent_messages_new RENAME TO intelligence_agent_messages;
        CREATE INDEX intelligence_agent_messages_agent
            ON intelligence_agent_messages(agent_id, id);
        CREATE INDEX intelligence_agent_messages_run ON intelligence_agent_messages(run_id)
            WHERE run_id IS NOT NULL;
        CREATE INDEX intelligence_agent_messages_reply_to ON intelligence_agent_messages(reply_to)
            WHERE reply_to IS NOT NULL;
        CREATE TRIGGER intelligence_agent_messages_reply_to
        BEFORE INSERT ON intelligence_agent_messages
        WHEN new.reply_to IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM intelligence_agent_messages
            WHERE id = new.reply_to AND agent_id = new.agent_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'A reply answers a message of the same conversation.');
        END;
        CREATE TRIGGER intelligence_agent_messages_unread AFTER INSERT ON intelligence_agent_messages
        WHEN new.kind NOT IN ('codeExecution', 'documentation', 'memory', 'askedAgent',
            'askedByAgent', 'note', 'woke', 'rebuiltContext')
        BEGIN
            UPDATE intelligence_agents
            SET agent_unread = (new.sender <> 'user'), last_message_at = new.sent_at
            WHERE id = new.agent_id;
        END;",
    },
    // What the user wrote about themselves for their agents to read: never
    // empty, like their name.
    Migration {
        name: "profile_bio",
        sql: "ALTER TABLE profile ADD COLUMN bio TEXT CHECK (bio <> '');",
    },
    // Articles stop being saved: every article is a feed's, so workspaces,
    // article folders and article tags go, the hooks and cues on saving and
    // tagging one, and the soft-deleted rows of pages once saved by hand. With
    // only tasks left to tag, a tag's kind goes too.
    Migration {
        name: "articles_only_from_feeds",
        sql: "
        DROP TABLE article_tags;
        DROP TABLE workspace_articles;
        DROP TABLE article_folders;

        DELETE FROM extension_hooks WHERE hook IN ('article:save', 'article:tagged');
        DELETE FROM intelligence_agent_cues WHERE hook IN ('article:save', 'article:tagged');

        DROP TRIGGER articles_search_ad;
        DROP TRIGGER articles_search_au;
        DROP TRIGGER articles_feed_corpus_ai;
        DROP TRIGGER articles_feed_corpus_ad;
        DROP TRIGGER articles_feed_corpus_au;
        DROP INDEX articles_feed_url;
        DROP INDEX articles_url;
        DELETE FROM search_fts WHERE rowid IN
            (SELECT id FROM search_entries WHERE entity_kind = 'article');
        DELETE FROM search_entries WHERE entity_kind = 'article';
        CREATE TABLE articles_new (
            id INTEGER PRIMARY KEY,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
            read BOOLEAN NOT NULL DEFAULT FALSE,
            url TEXT NOT NULL,
            title TEXT NOT NULL,
            author TEXT,
            published_at INTEGER NOT NULL DEFAULT 0,
            contents TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            modified INTEGER NOT NULL DEFAULT (unixepoch()),
            published_precision TEXT NOT NULL DEFAULT 'time',
            snapshot_state TEXT NOT NULL CHECK (snapshot_state IN ('captured', 'failed')),
            snapshot_at INTEGER NOT NULL,
            final_url TEXT,
            page_title TEXT,
            truncated BOOLEAN,
            failure_kind TEXT,
            failure_text TEXT,
            failure_code INTEGER,
            CHECK (contents = '' OR snapshot_state = 'captured'),
            CHECK ((final_url IS NOT NULL) = (snapshot_state = 'captured')),
            CHECK ((truncated IS NOT NULL) = (snapshot_state = 'captured')),
            CHECK ((failure_kind IS NOT NULL) = (snapshot_state = 'failed'))
        );
        INSERT INTO articles_new (id, deleted, feed_id, read, url, title, author, published_at,
                contents, created_at, modified, published_precision, snapshot_state, snapshot_at,
                final_url, page_title, truncated, failure_kind, failure_text, failure_code)
            SELECT id, deleted, feed_id, read, url, title, author, published_at, contents,
                created_at, modified, published_precision, snapshot_state, snapshot_at,
                final_url, page_title, truncated, failure_kind, failure_text, failure_code
            FROM articles WHERE feed_id IS NOT NULL;
        DROP TABLE articles;
        ALTER TABLE articles_new RENAME TO articles;
        CREATE INDEX articles_feed_url ON articles(feed_id, url);
        CREATE TRIGGER articles_feed_corpus_ai AFTER INSERT ON articles
        WHEN new.deleted = 0 BEGIN
            INSERT INTO feed_corpus_fts (rowid, title, author, contents)
                VALUES (new.id, new.title, new.author, new.contents);
        END;
        CREATE TRIGGER articles_feed_corpus_ad AFTER DELETE ON articles BEGIN
            DELETE FROM feed_corpus_fts WHERE rowid = old.id;
        END;
        CREATE TRIGGER articles_feed_corpus_au
        AFTER UPDATE OF title, author, contents, deleted ON articles BEGIN
            DELETE FROM feed_corpus_fts WHERE rowid = old.id;
            INSERT INTO feed_corpus_fts (rowid, title, author, contents)
                SELECT new.id, new.title, new.author, new.contents
                WHERE new.deleted = 0;
        END;

        DELETE FROM tags WHERE entity = 'article';
        CREATE TABLE tags_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            sort_order INTEGER NOT NULL DEFAULT 0,
            UNIQUE (workspace_id, name COLLATE NOCASE),
            UNIQUE (workspace_id, id)
        );
        INSERT INTO tags_new (id, workspace_id, name, created_at, sort_order)
            SELECT id, workspace_id, name, created_at, sort_order FROM tags;
        CREATE TABLE task_tags_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            task_id INTEGER NOT NULL,
            UNIQUE (tag_id, task_id),
            FOREIGN KEY (workspace_id, tag_id) REFERENCES tags(workspace_id, id) ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, task_id) REFERENCES tasks(workspace_id, id)
                ON DELETE CASCADE
        );
        INSERT INTO task_tags_new (id, workspace_id, tag_id, task_id)
            SELECT id, workspace_id, tag_id, task_id FROM task_tags;
        CREATE TABLE saved_task_view_tags_new (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL,
            view_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            tag_match TEXT NOT NULL,
            UNIQUE (view_id, tag_id),
            FOREIGN KEY (workspace_id, view_id) REFERENCES saved_task_views(workspace_id, id)
                ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, tag_id) REFERENCES tags(workspace_id, id) ON DELETE CASCADE
        );
        INSERT INTO saved_task_view_tags_new (id, workspace_id, view_id, tag_id, tag_match)
            SELECT id, workspace_id, view_id, tag_id, tag_match FROM saved_task_view_tags;
        DROP TABLE saved_task_view_tags;
        DROP TABLE task_tags;
        DROP TABLE tags;
        PRAGMA legacy_alter_table = ON;
        ALTER TABLE tags_new RENAME TO tags;
        ALTER TABLE task_tags_new RENAME TO task_tags;
        ALTER TABLE saved_task_view_tags_new RENAME TO saved_task_view_tags;
        PRAGMA legacy_alter_table = OFF;
        CREATE INDEX task_tags_task ON task_tags(task_id);",
    },
    // An actor session has one activity for its whole life, reopened each
    // time it works again, rather than one per stretch of work. A session's
    // rows become its first, so its id stays, holding what the newest says;
    // the rest go. The unread trigger is set aside while they merge, so
    // copying an old outcome marks no session unread.
    Migration {
        name: "one_activity_per_actor_session",
        sql: "
        DROP INDEX activities_one_open_actor_session;
        DROP TRIGGER activities_actor_session_unread;

        UPDATE activities AS kept SET (title, status, progress, ended_at) =
            (SELECT newest.title, newest.status, newest.progress, newest.ended_at
             FROM activities AS newest
             WHERE newest.actor_session_id = kept.actor_session_id
             ORDER BY newest.id DESC LIMIT 1)
        WHERE kept.id = (SELECT MIN(first.id) FROM activities AS first
            WHERE first.actor_session_id = kept.actor_session_id);

        DELETE FROM activities WHERE id > (SELECT MIN(first.id) FROM activities AS first
            WHERE first.actor_session_id = activities.actor_session_id);

        CREATE UNIQUE INDEX activities_actor_session ON activities(actor_session_id);

        CREATE TRIGGER activities_actor_session_unread AFTER UPDATE OF status ON activities
        WHEN new.status IN ('succeeded', 'failed')
        BEGIN
            UPDATE actor_sessions SET unread = TRUE WHERE id = new.actor_session_id;
        END;",
    },
    // Documents file into folders, which nest without limit. A document in
    // no folder is in the Inbox, where every existing one stays; a journal
    // is never filed. A document's folder is a live one of its workspace.
    Migration {
        name: "document_folders",
        sql: "
        CREATE TABLE document_folders (
            id INTEGER PRIMARY KEY,
            workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            parent_id INTEGER CHECK (parent_id IS NOT id),
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            deleted BOOLEAN NOT NULL DEFAULT FALSE,
            UNIQUE (workspace_id, id),
            FOREIGN KEY (workspace_id, parent_id) REFERENCES document_folders(workspace_id, id)
                ON DELETE CASCADE
        );
        CREATE INDEX document_folders_parent ON document_folders(parent_id)
            WHERE parent_id IS NOT NULL;

        ALTER TABLE documents ADD COLUMN folder_id INTEGER
            REFERENCES document_folders(id) ON DELETE SET NULL
            CHECK (folder_id IS NULL OR document_type = 'normal');
        CREATE INDEX documents_folder ON documents(folder_id) WHERE folder_id IS NOT NULL;

        CREATE TRIGGER documents_folder_in_workspace
        BEFORE INSERT ON documents
        WHEN new.folder_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM document_folders WHERE id = new.folder_id
                AND workspace_id = new.workspace_id AND deleted = FALSE
        )
        BEGIN
            SELECT RAISE(ABORT, 'A document is filed only in a live folder of its workspace.');
        END;

        CREATE TRIGGER documents_folder_in_workspace_au
        BEFORE UPDATE OF folder_id, workspace_id ON documents
        WHEN new.folder_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM document_folders WHERE id = new.folder_id
                AND workspace_id = new.workspace_id AND deleted = FALSE
        )
        BEGIN
            SELECT RAISE(ABORT, 'A document is filed only in a live folder of its workspace.');
        END;",
    },
];

#[cfg(test)]
mod tests;
