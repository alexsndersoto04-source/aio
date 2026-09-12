// Helper de BD para moon local (usa el cliente Node `pg`).
//   node db.mjs create-db   -> crea la base moon si no existe
//   node db.mjs reset       -> borra todas las tablas de public en moon
//   node db.mjs query "SQL" -> ejecuta SQL en moon e imprime el resultado
import pg from "pg";

const HOST = process.env.MOON_PG_HOST || "127.0.0.1";
const PORT = Number(process.env.MOON_PG_PORT || 5432);
const USER = process.env.MOON_PG_USER || "moon";
const cmd = process.argv[2];

async function main() {
  if (cmd === "create-db") {
    const admin = new pg.Client({ host: HOST, port: PORT, user: USER, database: "postgres" });
    await admin.connect();
    const r = await admin.query("SELECT 1 FROM pg_database WHERE datname = 'moon'");
    if (r.rowCount === 0) {
      await admin.query("CREATE DATABASE moon");
      console.log("BD moon creada");
    } else {
      console.log("BD moon ya existe");
    }
    await admin.end();
    return;
  }
  const c = new pg.Client({ host: HOST, port: PORT, user: USER, database: "moon" });
  await c.connect();
  if (cmd === "reset") {
    const t = await c.query(
      "SELECT tablename FROM pg_tables WHERE schemaname = 'public'"
    );
    for (const row of t.rows) {
      await c.query(`DROP TABLE IF EXISTS public.${row.tablename} CASCADE`);
    }
    console.log(`BD vaciada (${t.rowCount} tablas eliminadas)`);
  } else if (cmd === "query") {
    const r = await c.query(process.argv[3] || "SELECT 1");
    console.log(JSON.stringify(r.rows, null, 2));
  } else {
    console.error("uso: node db.mjs <create-db|reset|query SQL>");
    process.exit(2);
  }
  await c.end();
}

main().catch((e) => {
  console.error("ERROR:", e.message);
  process.exit(1);
});
