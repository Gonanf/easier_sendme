use anyhow::{Ok, Result};
use iroh::{protocol::Router, Endpoint, Watcher};
use iroh_blobs::{api::{blobs::{AddPathOptions, ExportMode, ExportOptions, ImportMode}, remote::GetProgressItem, tags::TagInfo}, format::collection::Collection, get::{self, Stats}, store::{fs::FsStore, mem::MemStore}, ticket::BlobTicket, BlobFormat, BlobsProtocol, Hash};
use n0_future::{BufferedStreamExt, StreamExt};
use walkdir::WalkDir;
use std::{ffi::OsString, fs, path::{self, PathBuf}};
use clap::Parser;
use tokio::{fs::create_dir, sync::mpsc};

fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>());
}

#[derive(Parser)]
#[command(version, about)]
struct Comandos{
    path: PathBuf,
    hash: Option<BlobTicket>,
    out: Option<PathBuf>,
    #[clap(short,long)]
    dir: Option<PathBuf>
}

#[tokio::main]
async fn main() -> Result<()> {
    let comandos = Comandos::parse();
    
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;

    let store = FsStore::load(std::path::absolute(&comandos.path)?).await?;
    
    println!("{:?}",comandos.hash);
    if let Some(res) = comandos.hash {
        let hash = res.hash_and_format();
        let local = store.remote().local(hash).await?;
        if !local.is_complete(){
            let addr = res.node_addr().clone();
            dbg!(addr.clone());
            let con = endpoint.connect(addr, iroh_blobs::ALPN).await?;  
            dbg!(con.clone());
            let g = store.remote().execute_get(con, local.missing()).await?;
            println!("Completado en: {}/{}b",g.elapsed.as_secs(),g.total_bytes_read());
        }
        let collection = Collection::load(hash.hash, store.as_ref()).await?;
        for (name,hash) in collection.iter(){
            let mut path_text = OsString::new();
            if let Some(out) = comandos.out.clone(){
                if !out.exists() {create_dir(out.clone()).await?;}
                path_text = out.canonicalize().unwrap().as_os_str().into();
                path_text.push("/");
            }
            else{
                path_text = std::env::current_dir()?.as_os_str().into();
                path_text.push("/");
            }
            path_text.push(name);
            //TODO: Realizar verificaciones
            dbg!(path_text.clone());
            let path: PathBuf = path_text.into();
            let mut stream_export = store.export_with_opts(ExportOptions{ hash: *hash, mode: ExportMode::Copy, target: path }).await?;
            dbg!(stream_export);
        }
        return Ok(());
    }
    let blobs: BlobsProtocol  = BlobsProtocol::new(&store, endpoint.clone(), None);

    let router = Router::builder(endpoint.clone()).accept(iroh_blobs::ALPN, blobs.clone()).spawn();
    

    if let Some(p) = comandos.dir {
    let can_p = p.canonicalize()?.clone();
    let archivos =  WalkDir::new(can_p.clone()).into_iter().filter_map(|f| {
        let f = f.ok()?;
        dbg!(f.clone());


        if !f.file_type().is_file(){
           return None;
        }
        let name = f.file_name().to_os_string();
        let ph  = f.clone().into_path();
        println!("{:?} {:?}", name, ph);

         Some((name,ph))
    }).collect::<Vec<(OsString,PathBuf)>>();

    dbg!(archivos.clone());

    let res = n0_future::stream::iter(archivos).map(
        |(f,p)| {
            let storage = blobs.store();
            let blob = storage.add_path_with_opts(AddPathOptions{
                path: p, 
                format: BlobFormat::Raw, 
                mode: ImportMode::TryReference });
            async move {return (f.into_string().unwrap(),blob.await.unwrap().hash)}
        }   
    ).buffered_unordered(1).collect::<Vec<(String,Hash)>>().await;

    let coleccion = Collection::from_iter(res);
    dbg!();
    let tag = coleccion.clone().store(&store).await?;
    let addr = router.endpoint().node_addr().initialized().await;
    let ticket = BlobTicket::new(addr,*tag.hash(),BlobFormat::HashSeq);
    println!("{:?}",coleccion);
    println!(" TICKET: {ticket}");
    } 
    //println!("Hashes: {:?}", store.list().hashes().await?);
    tokio::signal::ctrl_c().await;

    router.shutdown().await?;
    Ok(())
}
