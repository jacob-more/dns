/// This application is intended to help test the code that has been written so far. It is a simple
/// command line application that will take input from stdin and the command line and make queries
/// for various RTyes at each name.
use std::{env, io::IsTerminal, sync::Arc, time::Instant};

use dns_cache::asynchronous::async_main_cache::AsyncMainTreeCache;
use dns_client::DNSAsyncClient;
use dns_lib::{interface::{cache::{main_cache::AsyncMainCache, MetaAuth}, client::{AsyncClient, Context, QNameMinimization}}, query::question::Question, resource_record::{rclass::RClass, rtype::RType}, types::c_domain_name::CDomainName};
use futures::StreamExt;

const SEARCH_RTYPES: [RType; 35] = [
    RType::A,
    RType::AAAA,
    RType::AFSDB,
    RType::AMTRELAY,
    RType::ANY,
    RType::APL,
    RType::AXFR,
    RType::CAA,
    RType::CERT,
    RType::CNAME,
    RType::CSYNC,
    RType::DNAME,
    RType::DNSKEY,
    RType::HINFO,
    RType::MAILA,
    RType::MAILB,
    RType::MB,
    RType::MD,
    RType::MF,
    RType::MG,
    RType::MINFO,
    RType::MR,
    RType::MX,
    RType::NAPTR,
    RType::NS,
    RType::NSEC,
    RType::NULL,
    RType::PTR,
    RType::RRSIG,
    RType::SOA,
    RType::SRV,
    RType::TLSA,
    RType::TSIG,
    RType::TXT,
    RType::WKS,
];

#[tokio::main]
async fn main() {
    env_logger::init();

    rustls::crypto::ring::default_provider().install_default().expect("Failed to set Ring as the default crypto provider");

    let cache = Arc::new(AsyncMainTreeCache::new());
    let mut file = tokio::fs::File::open("root.hints").await.expect("The file `root.hints` is missing. Run `fetch-iana.sh` and place the downloaded files into your working directory");
    cache.load_from_file(&mut file, MetaAuth::NotAuthoritativeBootstrap).await.unwrap();
    let client = Arc::new(DNSAsyncClient::new(cache).await);

    let mut search_domains = env::args()
        .skip(1)    /* first argument indicates the name of the program */
        .filter(|arg| arg != "--all-cache" && arg != "-ac")
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    
    // Reading from stdin allows lists of domain names to be piped into the application.
    if !std::io::stdin().is_terminal() {
        search_domains.extend(std::io::stdin()
            .lines()
            .filter_map(|maybe| match maybe {
                Ok(string) => Some(string),
                Err(_) => None,
            })
        );
    }

    if search_domains.is_empty() {
        panic!("At least one domain must be provided for the application to search for");
    }

    let search_domains = search_domains.into_iter()
        .map(|domain_str| {
            let mut domain = CDomainName::from_utf8(&domain_str).expect("All input domain names must be valid but '{domain_str}' is not");
            domain.make_fully_qualified().expect("All input domains must be able to be represented as valid fully-qualified names but '{domain_str}' could not");
            domain
        });

    test_many_dns(client.clone(), search_domains).await;

    client.cache()
        .get_domains().await
        .into_iter()
        .for_each(|domain| println!("{domain}"));
    client.close().await;
}

async fn test_many_dns(client: Arc<DNSAsyncClient>, domains: impl Iterator<Item = CDomainName>) {
    let total_start = Instant::now();

    let domain_rtype_pairs = domains.map(|domain| SEARCH_RTYPES.into_iter().map(move |rtype| (domain.clone(), rtype))).flatten();
    futures::stream::iter(domain_rtype_pairs).for_each_concurrent(None, |(domain, rtype)| {
        let client = client.clone();
        async move {
            let _ = tokio::spawn(query_dn(client.clone(), rtype, domain)).await;
        }
    }).await;

    let total_time = total_start.elapsed().as_millis();
    println!("Total Time For All Queries: {total_time} ms");
}

async fn query_dn(client: Arc<DNSAsyncClient>, rtype: RType, domain: CDomainName) {
    println!("Start: Query for '{domain}'");
    let question = Question::new(domain, rtype, RClass::Internet);

    let start = Instant::now();
    let response = DNSAsyncClient::query(client, Context::new(question.clone(), QNameMinimization::All { primary_minimization_limit: 10, ns_minimization_limit: 8, sub_ns_minimization_limit: 6 })).await;
    let end = Instant::now();
    let total_time = (end - start).as_millis();
    println!("Query for '{question}':\nTotal Time: {total_time} ms\nAnswer: {:?}\n", response);
}
