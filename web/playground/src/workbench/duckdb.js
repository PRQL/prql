import * as duckdb from "@duckdb/duckdb-wasm";

export async function init() {
  const JSDELIVR_BUNDLES = duckdb.getJsDelivrBundles();

  // Select a bundle based on browser checks
  const bundle = await duckdb.selectBundle(JSDELIVR_BUNDLES);

  const worker_url = URL.createObjectURL(
    new Blob([`importScripts("${bundle.mainWorker}");`], {
      type: "text/javascript",
    }),
  );

  // Instantiate the asynchronous version of DuckDB-wasm
  const worker = new Worker(worker_url);
  const logger = new duckdb.ConsoleLogger();
  const db = new duckdb.AsyncDuckDB(logger, worker);
  await db.instantiate(bundle.mainModule, bundle.pthreadWorker);
  URL.revokeObjectURL(worker_url);

  await registerChinook(db);

  return db;
}

export const CHINOOK_TABLES = [
  "albums",
  "artists",
  "customers",
  "employees",
  "genres",
  "invoice_items",
  "invoices",
  "media_types",
  "playlists",
  "playlist_track",
  "tracks",
];

async function registerChinook(db) {
  const baseUrl = `${window.location.href}/data/chinook`;

  await Promise.all(
    CHINOOK_TABLES.map(async (table) => {
      const res = await fetch(`${baseUrl}/${table}.csv`);
      const text = await res.text();

      db.registerFileText(`${table}.csv`, text);
    }),
  );

  const c = await db.connect();
  for (const table of CHINOOK_TABLES) {
    await c.insertCSVFromPath(`${table}.csv`, {
      name: table,
      detect: true,
      header: true,
    });
  }
  c.close();
}
