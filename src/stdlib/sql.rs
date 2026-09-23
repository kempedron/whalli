use crate::heap::Obj;
use crate::value::{NativeResult, Value};
use crate::vm::VM;
use mysql::prelude::*;
use mysql::{Conn as MyConn, Value as MyValue};
use parking_lot::Mutex;
use postgres::types::{ToSql as PgToSql, Type as PgType};
use postgres::{Client as PgClient, NoTls};
use rusqlite::types::{ToSqlOutput, ValueRef};
use rusqlite::{Connection as SqliteConn, ToSql as SqliteToSql};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub enum DbConnection {
    Sqlite(SqliteConn),
    Postgres(PgClient),
    Mysql(MyConn),
}

pub struct DatabaseManager {
    next_id: AtomicUsize,
    conns: parking_lot::RwLock<HashMap<usize, Arc<Mutex<DbConnection>>>>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(1),
            conns: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    pub fn insert_sqlite(&self, conn: SqliteConn) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.conns
            .write()
            .insert(id, Arc::new(Mutex::new(DbConnection::Sqlite(conn))));
        id
    }

    pub fn insert_postgres(&self, client: PgClient) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.conns
            .write()
            .insert(id, Arc::new(Mutex::new(DbConnection::Postgres(client))));
        id
    }

    pub fn insert_mysql(&self, conn: MyConn) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.conns
            .write()
            .insert(id, Arc::new(Mutex::new(DbConnection::Mysql(conn))));
        id
    }

    pub fn get(&self, id: usize) -> Option<Arc<Mutex<DbConnection>>> {
        self.conns.read().get(&id).cloned()
    }

    pub fn close(&self, id: usize) -> bool {
        self.conns.write().remove(&id).is_some()
    }
}

static DB_MANAGER: std::sync::OnceLock<DatabaseManager> = std::sync::OnceLock::new();

pub fn db_manager() -> &'static DatabaseManager {
    DB_MANAGER.get_or_init(DatabaseManager::new)
}

#[derive(Debug, Clone)]
pub struct SqlParam(pub Value);

// rusqlite conversion
impl SqliteToSql for SqlParam {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match &self.0 {
            Value::Nil => Ok(ToSqlOutput::from(rusqlite::types::Null)),
            Value::Bool(b) => Ok(ToSqlOutput::from(*b)),
            Value::Int(i) => Ok(ToSqlOutput::from(*i)),
            Value::Float(f) => Ok(ToSqlOutput::from(*f)),
            Value::Str(s) => Ok(ToSqlOutput::from(s.as_str())),
            other => Ok(ToSqlOutput::from(format!("{}", other))),
        }
    }
}

// postgres conversion
impl PgToSql for SqlParam {
    fn to_sql(
        &self,
        ty: &PgType,
        out: &mut postgres::types::private::BytesMut,
    ) -> Result<postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match &self.0 {
            Value::Nil => Ok(postgres::types::IsNull::Yes),
            Value::Bool(b) => PgToSql::to_sql(b, ty, out),
            Value::Int(i) => {
                if *ty == PgType::INT4 {
                    PgToSql::to_sql(&(*i as i32), ty, out)
                } else if *ty == PgType::INT2 {
                    PgToSql::to_sql(&(*i as i16), ty, out)
                } else {
                    PgToSql::to_sql(i, ty, out)
                }
            }
            Value::Float(f) => {
                if *ty == PgType::FLOAT4 {
                    PgToSql::to_sql(&(*f as f32), ty, out)
                } else {
                    PgToSql::to_sql(f, ty, out)
                }
            }
            Value::Str(s) => PgToSql::to_sql(&s.as_str(), ty, out),
            other => {
                let formatted = format!("{}", other);
                PgToSql::to_sql(&formatted.as_str(), ty, out)
            }
        }
    }

    fn accepts(_ty: &PgType) -> bool {
        true
    }

    postgres::types::to_sql_checked!();
}

// mysql conversion
impl From<SqlParam> for MyValue {
    fn from(p: SqlParam) -> Self {
        match p.0 {
            Value::Nil => MyValue::NULL,
            Value::Bool(b) => MyValue::from(b),
            Value::Int(i) => MyValue::from(i),
            Value::Float(f) => MyValue::from(f),
            Value::Str(s) => MyValue::from(s.as_str()),
            other => MyValue::from(format!("{}", other)),
        }
    }
}

pub fn convert_sql_args(args_val: Option<&Value>, heap: &crate::heap::Heap) -> Vec<SqlParam> {
    let mut params = Vec::new();
    if let Some(Value::ObjRef(id)) = args_val {
        if let Ok(Obj::List(list)) = heap.get(*id) {
            for v in list {
                params.push(SqlParam(v));
            }
        }
    }
    params
}

// Translate '?' placeholders to '$1, $2, ...' for Postgres
pub fn translate_query_for_postgres(query: &str) -> String {
    let mut result = String::with_capacity(query.len() + 8);
    let mut param_index = 1;
    let mut in_quote = false;
    let mut quote_char = ' ';

    for ch in query.chars() {
        if in_quote {
            if ch == quote_char {
                in_quote = false;
            }
            result.push(ch);
        } else if ch == '\'' || ch == '"' {
            in_quote = true;
            quote_char = ch;
            result.push(ch);
        } else if ch == '?' {
            result.push('$');
            result.push_str(&param_index.to_string());
            param_index += 1;
        } else {
            result.push(ch);
        }
    }
    result
}

fn sqlite_value_to_whalli(val: ValueRef) -> Value {
    match val {
        ValueRef::Null => Value::Nil,
        ValueRef::Integer(i) => Value::Int(i),
        ValueRef::Real(f) => Value::Float(f),
        ValueRef::Text(bytes) => {
            let s = String::from_utf8_lossy(bytes).into_owned();
            Value::Str(Arc::new(s))
        }
        ValueRef::Blob(bytes) => {
            let s = String::from_utf8_lossy(bytes).into_owned();
            Value::Str(Arc::new(s))
        }
    }
}

fn pg_row_value_to_whalli(row: &postgres::Row, idx: usize) -> Value {
    let col = &row.columns()[idx];
    let ty = col.type_();

    if *ty == PgType::BOOL {
        if let Ok(Some(v)) = row.try_get::<_, Option<bool>>(idx) {
            return Value::Bool(v);
        }
    } else if *ty == PgType::INT2 {
        if let Ok(Some(v)) = row.try_get::<_, Option<i16>>(idx) {
            return Value::Int(v as i64);
        }
    } else if *ty == PgType::INT4 {
        if let Ok(Some(v)) = row.try_get::<_, Option<i32>>(idx) {
            return Value::Int(v as i64);
        }
    } else if *ty == PgType::INT8 {
        if let Ok(Some(v)) = row.try_get::<_, Option<i64>>(idx) {
            return Value::Int(v);
        }
    } else if *ty == PgType::FLOAT4 {
        if let Ok(Some(v)) = row.try_get::<_, Option<f32>>(idx) {
            return Value::Float(v as f64);
        }
    } else if *ty == PgType::FLOAT8 {
        if let Ok(Some(v)) = row.try_get::<_, Option<f64>>(idx) {
            return Value::Float(v);
        }
    } else if *ty == PgType::TEXT || *ty == PgType::VARCHAR || *ty == PgType::BPCHAR || *ty == PgType::NAME || *ty == PgType::JSON || *ty == PgType::JSONB {
        if let Ok(Some(v)) = row.try_get::<_, Option<String>>(idx) {
            return Value::Str(Arc::new(v));
        }
    }

    // Generic fallback to string or NULL
    if let Ok(Some(v)) = row.try_get::<_, Option<String>>(idx) {
        Value::Str(Arc::new(v))
    } else {
        Value::Nil
    }
}

fn mysql_value_to_whalli(val: MyValue) -> Value {
    match val {
        MyValue::NULL => Value::Nil,
        MyValue::Bytes(bytes) => {
            let s = String::from_utf8(bytes.clone())
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned());
            Value::Str(Arc::new(s))
        }
        MyValue::Int(i) => Value::Int(i),
        MyValue::UInt(u) => Value::Int(u as i64),
        MyValue::Float(f) => Value::Float(f as f64),
        MyValue::Double(d) => Value::Float(d),
        MyValue::Date(year, month, day, hour, min, sec, micro) => {
            let s = format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:06}", year, month, day, hour, min, sec, micro);
            Value::Str(Arc::new(s))
        }
        MyValue::Time(is_neg, days, hours, minutes, seconds, micro) => {
            let sign = if is_neg { "-" } else { "" };
            let s = format!("{}{}:{:02}:{:02}.{:06}", sign, days * 24 + hours as u32, minutes, seconds, micro);
            Value::Str(Arc::new(s))
        }
    }
}

pub fn db_exec_internal(
    conn_id: usize,
    query: &str,
    params: Vec<SqlParam>,
    heap: &crate::heap::Heap,
) -> Result<Value, String> {
    let conn_arc = db_manager()
        .get(conn_id)
        .ok_or_else(|| "Database connection not found or closed".to_string())?;

    let mut guard = conn_arc.lock();
    match &mut *guard {
        DbConnection::Sqlite(conn) => {
            let sql_params: Vec<&dyn SqliteToSql> = params.iter().map(|p| p as &dyn SqliteToSql).collect();
            let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
            let affected = stmt.execute(sql_params.as_slice()).map_err(|e| e.to_string())?;
            let last_id = conn.last_insert_rowid();

            let mut res_map = HashMap::new();
            res_map.insert("rows_affected".to_string(), Value::Int(affected as i64));
            res_map.insert("last_insert_id".to_string(), Value::Int(last_id));
            let r_id = heap.alloc(Obj::Map(res_map));
            Ok(Value::ObjRef(r_id))
        }
        DbConnection::Postgres(client) => {
            let pg_query = translate_query_for_postgres(query);
            let pg_params: Vec<&(dyn PgToSql + Sync)> = params.iter().map(|p| p as &(dyn PgToSql + Sync)).collect();
            let affected = client
                .execute(&pg_query, pg_params.as_slice())
                .map_err(|e| e.to_string())?;

            let mut res_map = HashMap::new();
            res_map.insert("rows_affected".to_string(), Value::Int(affected as i64));
            res_map.insert("last_insert_id".to_string(), Value::Int(0));
            let r_id = heap.alloc(Obj::Map(res_map));
            Ok(Value::ObjRef(r_id))
        }
        DbConnection::Mysql(conn) => {
            let my_params: Vec<MyValue> = params.into_iter().map(MyValue::from).collect();
            conn.exec_drop(query, my_params).map_err(|e| e.to_string())?;
            let affected = conn.affected_rows();
            let last_id = conn.last_insert_id();

            let mut res_map = HashMap::new();
            res_map.insert("rows_affected".to_string(), Value::Int(affected as i64));
            res_map.insert("last_insert_id".to_string(), Value::Int(last_id as i64));
            let r_id = heap.alloc(Obj::Map(res_map));
            Ok(Value::ObjRef(r_id))
        }
    }
}

pub fn db_query_internal(
    conn_id: usize,
    query: &str,
    params: Vec<SqlParam>,
    heap: &crate::heap::Heap,
) -> Result<Value, String> {
    let conn_arc = db_manager()
        .get(conn_id)
        .ok_or_else(|| "Database connection not found or closed".to_string())?;

    let mut guard = conn_arc.lock();
    match &mut *guard {
        DbConnection::Sqlite(conn) => {
            let sql_params: Vec<&dyn SqliteToSql> = params.iter().map(|p| p as &dyn SqliteToSql).collect();
            let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
            let col_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

            let mut rows = stmt.query(sql_params.as_slice()).map_err(|e| e.to_string())?;
            let mut rows_list = Vec::new();

            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                let mut row_map = HashMap::new();
                for (idx, name) in col_names.iter().enumerate() {
                    let val_ref = row.get_ref(idx).map_err(|e| e.to_string())?;
                    row_map.insert(name.clone(), sqlite_value_to_whalli(val_ref));
                }
                let row_id = heap.alloc(Obj::Map(row_map));
                rows_list.push(Value::ObjRef(row_id));
            }

            let list_id = heap.alloc(Obj::List(rows_list));
            Ok(Value::ObjRef(list_id))
        }
        DbConnection::Postgres(client) => {
            let pg_query = translate_query_for_postgres(query);
            let pg_params: Vec<&(dyn PgToSql + Sync)> = params.iter().map(|p| p as &(dyn PgToSql + Sync)).collect();
            let rows = client
                .query(&pg_query, pg_params.as_slice())
                .map_err(|e| e.to_string())?;

            let mut rows_list = Vec::new();
            for row in rows {
                let mut row_map = HashMap::new();
                for (idx, col) in row.columns().iter().enumerate() {
                    let name = col.name().to_string();
                    let val = pg_row_value_to_whalli(&row, idx);
                    row_map.insert(name, val);
                }
                let row_id = heap.alloc(Obj::Map(row_map));
                rows_list.push(Value::ObjRef(row_id));
            }

            let list_id = heap.alloc(Obj::List(rows_list));
            Ok(Value::ObjRef(list_id))
        }
        DbConnection::Mysql(conn) => {
            let my_params: Vec<MyValue> = params.into_iter().map(MyValue::from).collect();
            let result = conn.exec_iter(query, my_params).map_err(|e| e.to_string())?;
            let columns = result.columns().as_ref().to_vec();
            let col_names: Vec<String> = columns.iter().map(|c| c.name_str().to_string()).collect();

            let mut rows_list = Vec::new();
            for row in result {
                let mut row = row.map_err(|e| e.to_string())?;
                let mut row_map = HashMap::new();
                for (idx, name) in col_names.iter().enumerate() {
                    if let Some(my_val) = row.take(idx) {
                        row_map.insert(name.clone(), mysql_value_to_whalli(my_val));
                    } else {
                        row_map.insert(name.clone(), Value::Nil);
                    }
                }
                let row_id = heap.alloc(Obj::Map(row_map));
                rows_list.push(Value::ObjRef(row_id));
            }

            let list_id = heap.alloc(Obj::List(rows_list));
            Ok(Value::ObjRef(list_id))
        }
    }
}

pub fn db_query_row_internal(
    conn_id: usize,
    query: &str,
    params: Vec<SqlParam>,
    heap: &crate::heap::Heap,
) -> Result<Value, String> {
    let conn_arc = db_manager()
        .get(conn_id)
        .ok_or_else(|| "Database connection not found or closed".to_string())?;

    let mut guard = conn_arc.lock();
    match &mut *guard {
        DbConnection::Sqlite(conn) => {
            let sql_params: Vec<&dyn SqliteToSql> = params.iter().map(|p| p as &dyn SqliteToSql).collect();
            let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
            let col_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

            let mut rows = stmt.query(sql_params.as_slice()).map_err(|e| e.to_string())?;
            if let Some(row) = rows.next().map_err(|e| e.to_string())? {
                let mut row_map = HashMap::new();
                for (idx, name) in col_names.iter().enumerate() {
                    let val_ref = row.get_ref(idx).map_err(|e| e.to_string())?;
                    row_map.insert(name.clone(), sqlite_value_to_whalli(val_ref));
                }
                let row_id = heap.alloc(Obj::Map(row_map));
                Ok(Value::ObjRef(row_id))
            } else {
                Ok(Value::Nil)
            }
        }
        DbConnection::Postgres(client) => {
            let pg_query = translate_query_for_postgres(query);
            let pg_params: Vec<&(dyn PgToSql + Sync)> = params.iter().map(|p| p as &(dyn PgToSql + Sync)).collect();
            let opt_row = client
                .query_opt(&pg_query, pg_params.as_slice())
                .map_err(|e| e.to_string())?;

            if let Some(row) = opt_row {
                let mut row_map = HashMap::new();
                for (idx, col) in row.columns().iter().enumerate() {
                    let name = col.name().to_string();
                    let val = pg_row_value_to_whalli(&row, idx);
                    row_map.insert(name, val);
                }
                let row_id = heap.alloc(Obj::Map(row_map));
                Ok(Value::ObjRef(row_id))
            } else {
                Ok(Value::Nil)
            }
        }
        DbConnection::Mysql(conn) => {
            let my_params: Vec<MyValue> = params.into_iter().map(MyValue::from).collect();
            let mut result = conn.exec_iter(query, my_params).map_err(|e| e.to_string())?;
            let columns = result.columns().as_ref().to_vec();
            let col_names: Vec<String> = columns.iter().map(|c| c.name_str().to_string()).collect();

            if let Some(row_res) = result.next() {
                let mut row = row_res.map_err(|e| e.to_string())?;
                let mut row_map = HashMap::new();
                for (idx, name) in col_names.iter().enumerate() {
                    if let Some(my_val) = row.take(idx) {
                        row_map.insert(name.clone(), mysql_value_to_whalli(my_val));
                    } else {
                        row_map.insert(name.clone(), Value::Nil);
                    }
                }
                let row_id = heap.alloc(Obj::Map(row_map));
                Ok(Value::ObjRef(row_id))
            } else {
                Ok(Value::Nil)
            }
        }
    }
}

pub fn register(vm: &mut VM) -> Value {
    let mut sql_module = HashMap::new();

    // sql.open(driver: str, conn_str: str) -> (db: map | nil, err: str | nil)
    sql_module.insert(
        "open".to_string(),
        Value::Native(|args, vm| {
            if args.len() < 2 {
                let res = Value::Tuple(Arc::new(vec![
                    Value::Nil,
                    Value::Str(Arc::new("sql.open requires 2 arguments: (driver: str, conn_str: str)".to_string())),
                ]));
                return NativeResult::Return(res);
            }

            let driver = match &args[0] {
                Value::Str(s) => s.as_str().to_lowercase(),
                _ => {
                    let res = Value::Tuple(Arc::new(vec![
                        Value::Nil,
                        Value::Str(Arc::new("Driver name must be a string (e.g. 'sqlite', 'postgres', 'mysql')".to_string())),
                    ]));
                    return NativeResult::Return(res);
                }
            };

            let conn_str = match &args[1] {
                Value::Str(s) => s.as_str(),
                _ => {
                    let res = Value::Tuple(Arc::new(vec![
                        Value::Nil,
                        Value::Str(Arc::new("Connection string must be a string".to_string())),
                    ]));
                    return NativeResult::Return(res);
                }
            };

            match driver.as_str() {
                "sqlite" | "sqlite3" => {
                    let conn_result = if conn_str == ":memory:" {
                        SqliteConn::open_in_memory()
                    } else {
                        SqliteConn::open(conn_str)
                    };

                    match conn_result {
                        Ok(conn) => {
                            let conn_id = db_manager().insert_sqlite(conn);
                            let mut db_map = HashMap::new();
                            db_map.insert("db_id".to_string(), Value::Int(conn_id as i64));
                            db_map.insert("driver".to_string(), Value::Str(Arc::new("sqlite".to_string())));
                            let db_obj_id = vm.heap.alloc(Obj::Map(db_map));

                            let res = Value::Tuple(Arc::new(vec![Value::ObjRef(db_obj_id), Value::Nil]));
                            NativeResult::Return(res)
                        }
                        Err(e) => {
                            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                            NativeResult::Return(res)
                        }
                    }
                }
                "postgres" | "postgresql" | "pg" => {
                    match PgClient::connect(conn_str, NoTls) {
                        Ok(client) => {
                            let conn_id = db_manager().insert_postgres(client);
                            let mut db_map = HashMap::new();
                            db_map.insert("db_id".to_string(), Value::Int(conn_id as i64));
                            db_map.insert("driver".to_string(), Value::Str(Arc::new("postgres".to_string())));
                            let db_obj_id = vm.heap.alloc(Obj::Map(db_map));

                            let res = Value::Tuple(Arc::new(vec![Value::ObjRef(db_obj_id), Value::Nil]));
                            NativeResult::Return(res)
                        }
                        Err(e) => {
                            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                            NativeResult::Return(res)
                        }
                    }
                }
                "mysql" | "mariadb" => {
                    match MyConn::new(conn_str) {
                        Ok(conn) => {
                            let conn_id = db_manager().insert_mysql(conn);
                            let mut db_map = HashMap::new();
                            db_map.insert("db_id".to_string(), Value::Int(conn_id as i64));
                            db_map.insert("driver".to_string(), Value::Str(Arc::new("mysql".to_string())));
                            let db_obj_id = vm.heap.alloc(Obj::Map(db_map));

                            let res = Value::Tuple(Arc::new(vec![Value::ObjRef(db_obj_id), Value::Nil]));
                            NativeResult::Return(res)
                        }
                        Err(e) => {
                            let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(e.to_string()))]));
                            NativeResult::Return(res)
                        }
                    }
                }
                other => {
                    let err = format!("Unsupported SQL driver: '{}'. Currently supported: 'sqlite', 'postgres', 'mysql'", other);
                    let res = Value::Tuple(Arc::new(vec![Value::Nil, Value::Str(Arc::new(err))]));
                    NativeResult::Return(res)
                }
            }
        }),
    );

    let id = vm.heap.alloc(Obj::Map(sql_module));
    Value::ObjRef(id)
}
