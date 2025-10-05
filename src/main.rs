use anyhow::{Context, Ok, Result};
use clap::{Parser, Subcommand};
use iroh::{protocol::Router, Endpoint, SecretKey, Watcher};
use iroh_blobs::{
    BlobFormat, BlobsProtocol, Hash,
    api::{
        blobs::{AddPathOptions, ExportMode, ExportOptions, ImportMode},
        remote::GetProgressItem,
        tags::TagInfo,
    },
    format::collection::Collection,
    get::{self, Stats},
    store::{fs::FsStore, mem::MemStore},
    ticket::{self, BlobTicket},
};
use n0_future::{BufferedStreamExt, StreamExt};
use std::{
    ffi::OsString,
    fs::{self, create_dir_all},
    path::{self, Path, PathBuf},
};
use tokio::{
    fs::{create_dir, remove_dir_all},
    sync::mpsc,
};
use walkdir::WalkDir;

fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>());
}

#[derive(Parser)]
#[command(version, about)]
struct Comandos {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Send {
        path: PathBuf,
        database: Option<PathBuf>,
    },
    Receive {
        ticket: BlobTicket,
        path: Option<PathBuf>,
    },
}

async fn send(path: PathBuf, database: Option<PathBuf>) -> Result<()> {
    let mut rng = rand::rngs::OsRng;
    let key =  SecretKey::generate(&mut rng);
    let endpoint = Endpoint::builder().secret_key(key).discovery_n0().bind().await?;

    let database_path: PathBuf = match database {
        Some(a) => a.canonicalize()?,
        None => std::env::current_dir()?.join(PathBuf::from(format!(
            ".temp_sender_{:?}",
            path.file_name().unwrap().to_os_string()
        ))),
    };

    let store = FsStore::load(database_path.clone()).await?;

    let blobs: BlobsProtocol = BlobsProtocol::new(&store, endpoint.clone(), None);

    let router = Router::builder(endpoint.clone())
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();

    let archivos = WalkDir::new(path.canonicalize()?.clone())
        .into_iter()
        .filter_map(|f| {
            let f = f.ok()?;
            dbg!(f.clone());

            if !f.file_type().is_file() {
                return None;
            }
            let name = f.clone().into_path().strip_prefix(path.canonicalize().unwrap().parent()?).unwrap().as_os_str().to_owned();
            let ph = f.clone().into_path();

            println!("{:?} {:?}", name, ph);

            Some((name, ph))
        })
        .collect::<Vec<(OsString, PathBuf)>>();

    dbg!(archivos.clone());

    let res = n0_future::stream::iter(archivos)
        .map(|(f, p)| {
            let storage = blobs.store();
            let blob = storage.add_path_with_opts(AddPathOptions {
                path: p,
                format: BlobFormat::Raw,
                mode: ImportMode::TryReference,
            });
            async move { return (f.into_string().unwrap(), blob.await.unwrap().hash) }
        })
        .buffered_unordered(num_cpus::get())
        .collect::<Vec<(String, Hash)>>()
        .await;

    let coleccion = Collection::from_iter(res);
    dbg!();
    let tag = coleccion.clone().store(&store).await?;
    let addr = router.endpoint().node_addr().initialized().await;
    let ticket = BlobTicket::new(addr, *tag.hash(), BlobFormat::HashSeq);
    println!("{:?}", coleccion);
    println!(" TICKET: {ticket}");

    tokio::signal::ctrl_c().await;

    remove_dir_all(database_path).await?;
    router.shutdown().await?;
    Ok(())
}

async fn receive(path: Option<PathBuf>, ticket: BlobTicket) -> Result<()> {
    let mut rng = rand::rngs::OsRng;
    let key =  SecretKey::generate(&mut rng);
    let endpoint = Endpoint::builder().secret_key(key).discovery_n0().bind().await?;

    let temp_path =
        std::env::current_dir()?.join(PathBuf::from(format!(".temp_{}", ticket.hash().to_hex())));
    dbg!(temp_path.clone());

    if !temp_path.exists() {
        create_dir_all(temp_path.clone())?;
    }

    let store = FsStore::load(temp_path.clone()).await?;
    let hash = ticket.hash_and_format();
    let local = store.remote().local(hash).await?;

    if !local.is_complete() {
        let addr = ticket.node_addr();
        dbg!(addr.clone());
        let con = endpoint.connect(addr.clone(), iroh_blobs::ALPN).await?;
        dbg!(con.clone());
        let g = store.remote().execute_get(con, local.missing()).await?;
        println!(
            "Completado en: {}/{}b",
            g.elapsed.as_secs(),
            g.total_bytes_read()
        );
    }
    let collection = Collection::load(hash.hash, store.as_ref()).await?;
    let first_name = collection.iter().next();

    for (name, hash) in collection.iter() {
    let current_path = std::env::current_dir()?;

            let rel_path = 
         if let Some(name_path) = path.clone() {
            println!("exporting to {:?}", name_path);
            name_path.canonicalize()?
        }
        else{
        println!("exporting to {:?}", current_path);
        current_path
        };
        let file_path = rel_path.join(PathBuf::from(name));
        dbg!(file_path.clone());
        let mut stream_export = store
            .export_with_opts(ExportOptions {
                hash: *hash,
                mode: ExportMode::Copy,
                target: file_path,
            })
            .await?;
        dbg!(stream_export);
    }
    remove_dir_all(temp_path).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let comandos = Comandos::parse();
    match comandos.operation {
        Operation::Send { path, database } => send(path, database).await?,
        Operation::Receive { path, ticket } => receive(path, ticket).await?,
    }
    Ok(())
}
