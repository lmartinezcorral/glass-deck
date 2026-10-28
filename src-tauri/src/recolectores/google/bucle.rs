use reqwest::Url;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const TIEMPO_LIMITE: Duration = Duration::from_secs(5 * 60);
const SONDEO: Duration = Duration::from_millis(100);
const TIEMPO_SOCKET: Duration = Duration::from_secs(5);
const LARGO_MAX_PETICION: usize = 8 * 1024;
const LARGO_MAX_CODIGO: usize = 2_048;

const PAGINA_LISTA: &str = "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>Glassdeck</title></head>\
<body style=\"font-family:system-ui,sans-serif;padding:3rem;background:#111;color:#eee\"><h1>Glassdeck conectado</h1>\
<p>Puedes cerrar esta ventana.</p></body></html>";
const PAGINA_CANCELADA: &str = "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>Glassdeck</title></head>\
<body style=\"font-family:system-ui,sans-serif;padding:3rem;background:#111;color:#eee\"><h1>No se conectó la cuenta</h1>\
<p>Puedes cerrar esta ventana y volver a intentarlo desde Glassdeck.</p></body></html>";

#[derive(Debug, PartialEq, Eq)]
pub enum FinBucle {
    Codigo(String),
    Denegado(String),
    TiempoAgotado,
    Cancelado,
    Error,
}

/// Servidor HTTP de un solo uso para la redirección de OAuth (GD-MAIL-01). Solo escucha en 127.0.0.1;
/// no pasa por `conexion` porque es entrante.
pub struct ServidorBucle {
    escucha: TcpListener,
    puerto: u16,
}

impl ServidorBucle {
    pub fn abrir() -> std::io::Result<Self> {
        let escucha = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        escucha.set_nonblocking(true)?;
        let puerto = escucha.local_addr()?.port();
        Ok(Self { escucha, puerto })
    }

    pub fn redireccion(&self) -> String {
        format!("http://127.0.0.1:{}", self.puerto)
    }

    /// Consume el servidor: al volver, el puerto queda cerrado. Las peticiones a otro camino, con otro método
    /// o con otro `state` reciben un error y no terminan la espera.
    pub fn esperar(self, estado: &str, limite: Duration, cancelado: &AtomicBool) -> FinBucle {
        let fin = Instant::now() + limite;
        loop {
            if cancelado.load(Ordering::Relaxed) {
                return FinBucle::Cancelado;
            }
            if Instant::now() >= fin {
                return FinBucle::TiempoAgotado;
            }
            match self.escucha.accept() {
                Ok((flujo, origen)) => {
                    if !origen.ip().is_loopback() {
                        continue;
                    }
                    if let Some(f) = atender(flujo, estado) {
                        return f;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(SONDEO),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => return FinBucle::Error,
            }
        }
    }
}

fn leer_cabecera(flujo: &mut TcpStream) -> Option<String> {
    let mut datos = Vec::with_capacity(1_024);
    let mut bufer = [0u8; 1_024];
    while datos.len() < LARGO_MAX_PETICION && !datos.windows(4).any(|v| v == b"\r\n\r\n") {
        match flujo.read(&mut bufer) {
            Ok(0) => break,
            Ok(n) => datos.extend_from_slice(&bufer[..n]),
            Err(_) => break,
        }
    }
    let texto = String::from_utf8_lossy(&datos).into_owned();
    texto.contains("\r\n").then_some(texto)
}

fn atender(mut flujo: TcpStream, estado: &str) -> Option<FinBucle> {
    let _ = flujo.set_nonblocking(false);
    let _ = flujo.set_read_timeout(Some(TIEMPO_SOCKET));
    let _ = flujo.set_write_timeout(Some(TIEMPO_SOCKET));
    let (fin, respuesta) = match leer_cabecera(&mut flujo) {
        Some(peticion) => evaluar(&peticion, estado),
        None => (None, responder(400, "Bad Request", "")),
    };
    let _ = flujo.write_all(respuesta.as_bytes());
    let _ = flujo.flush();
    let _ = flujo.shutdown(Shutdown::Both);
    fin
}

fn responder(codigo: u16, razon: &str, cuerpo: &str) -> String {
    format!(
        "HTTP/1.1 {codigo} {razon}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n\
Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    )
}

fn texto_seguro(s: &str, largo: usize) -> bool {
    !s.is_empty() && s.len() <= largo && s.chars().all(|c| c.is_ascii_graphic())
}

/// Decide qué hacer con una petición; separada del socket para probarla sin red.
pub fn evaluar(peticion: &str, estado: &str) -> (Option<FinBucle>, String) {
    let linea = peticion.lines().next().unwrap_or_default();
    let mut partes = linea.split(' ');
    let (metodo, destino) = (partes.next().unwrap_or_default(), partes.next().unwrap_or_default());
    if metodo != "GET" {
        return (None, responder(405, "Method Not Allowed", ""));
    }
    if !destino.starts_with('/') {
        return (None, responder(400, "Bad Request", ""));
    }
    let Ok(u) = Url::parse(&format!("http://127.0.0.1{destino}")) else {
        return (None, responder(400, "Bad Request", ""));
    };
    if u.path() != "/" {
        return (None, responder(404, "Not Found", ""));
    }
    let p: HashMap<String, String> = u.query_pairs().into_owned().collect();
    if estado.is_empty() || p.get("state").map(String::as_str) != Some(estado) {
        return (None, responder(400, "Bad Request", ""));
    }
    if let Some(codigo) = p.get("code").filter(|c| texto_seguro(c, LARGO_MAX_CODIGO)) {
        return (Some(FinBucle::Codigo(codigo.clone())), responder(200, "OK", PAGINA_LISTA));
    }
    if let Some(error) = p.get("error") {
        let error: String = error.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(64).collect();
        return (Some(FinBucle::Denegado(error)), responder(200, "OK", PAGINA_CANCELADA));
    }
    (None, responder(400, "Bad Request", ""))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::net::SocketAddr;

    const ESTADO: &str = "estado-ficticio-123";

    #[test]
    fn solo_acepta_get_a_la_raiz_con_el_state_correcto() {
        let (fin, r) = evaluar(&format!("POST /?state={ESTADO}&code=abc HTTP/1.1\r\n\r\n"), ESTADO);
        assert!(fin.is_none() && r.starts_with("HTTP/1.1 405"));
        let (fin, r) = evaluar(&format!("GET /favicon.ico?state={ESTADO}&code=abc HTTP/1.1\r\n\r\n"), ESTADO);
        assert!(fin.is_none() && r.starts_with("HTTP/1.1 404"));
        let (fin, r) = evaluar("GET /?state=otro&code=abc HTTP/1.1\r\n\r\n", ESTADO);
        assert!(fin.is_none() && r.starts_with("HTTP/1.1 400"));
        let (fin, _) = evaluar("GET /?code=abc HTTP/1.1\r\n\r\n", ESTADO);
        assert!(fin.is_none());
        let (fin, _) = evaluar("GET http://example.com/?state=x HTTP/1.1\r\n\r\n", ESTADO);
        assert!(fin.is_none());
        let (fin, r) = evaluar(&format!("GET /?state={ESTADO}&code=4%2Fcodigo-ficticio&scope=x HTTP/1.1\r\n\r\n"), ESTADO);
        assert_eq!(fin, Some(FinBucle::Codigo("4/codigo-ficticio".into())));
        assert!(r.starts_with("HTTP/1.1 200") && r.contains("Puedes cerrar esta ventana"));
        let (fin, _) = evaluar(&format!("GET /?state={ESTADO}&error=access_denied HTTP/1.1\r\n\r\n"), ESTADO);
        assert_eq!(fin, Some(FinBucle::Denegado("access_denied".into())));
        let (fin, _) = evaluar("GET /?state=&code=abc HTTP/1.1\r\n\r\n", "");
        assert!(fin.is_none(), "un state vacío nunca se acepta");
    }

    fn pedir(destino: SocketAddr, peticion: &str) -> String {
        let mut s = TcpStream::connect_timeout(&destino, Duration::from_secs(3)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(peticion.as_bytes()).unwrap();
        let mut r = String::new();
        let _ = s.read_to_string(&mut r);
        r
    }

    #[test]
    fn atiende_una_sola_peticion_valida_y_cierra_el_puerto() {
        let servidor = ServidorBucle::abrir().unwrap();
        let redireccion = servidor.redireccion();
        assert!(redireccion.starts_with("http://127.0.0.1:"));
        let destino: SocketAddr = redireccion.trim_start_matches("http://").parse().unwrap();
        assert!(destino.ip().is_loopback());
        let hilo = std::thread::spawn(move || servidor.esperar(ESTADO, Duration::from_secs(20), &AtomicBool::new(false)));
        assert!(pedir(destino, "GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n").starts_with("HTTP/1.1 404"));
        assert!(pedir(destino, "GET /?state=ajeno&code=x HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 400"));
        assert!(pedir(destino, &format!("DELETE /?state={ESTADO}&code=x HTTP/1.1\r\n\r\n")).starts_with("HTTP/1.1 405"));
        let ok = pedir(destino, &format!("GET /?state={ESTADO}&code=codigo-ficticio HTTP/1.1\r\n\r\n"));
        assert!(ok.starts_with("HTTP/1.1 200") && ok.contains("Puedes cerrar esta ventana"));
        assert_eq!(hilo.join().unwrap(), FinBucle::Codigo("codigo-ficticio".into()));
        assert!(TcpStream::connect_timeout(&destino, Duration::from_secs(3)).is_err(), "el puerto debe quedar cerrado");
    }

    #[test]
    fn se_cierra_por_tiempo_o_al_cancelar() {
        let servidor = ServidorBucle::abrir().unwrap();
        let destino: SocketAddr = servidor.redireccion().trim_start_matches("http://").parse().unwrap();
        let inicio = Instant::now();
        assert_eq!(servidor.esperar(ESTADO, Duration::from_millis(300), &AtomicBool::new(false)), FinBucle::TiempoAgotado);
        assert!(inicio.elapsed() < Duration::from_secs(3));
        assert!(TcpStream::connect_timeout(&destino, Duration::from_secs(3)).is_err());
        let servidor = ServidorBucle::abrir().unwrap();
        assert_eq!(servidor.esperar(ESTADO, Duration::from_secs(30), &AtomicBool::new(true)), FinBucle::Cancelado);
    }
}
