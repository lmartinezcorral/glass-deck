//! QA de fase 1 contra los contadores de Windows en el equipo real (GD-SYS-01, 03, 05, 06, 07 y 08).
//! Usan el mismo `Recolector` que la app, a la cadencia de los intervalos por defecto, y leen PDH en
//! el mismo proceso y en el mismo instante. Se lanzan con `poc/qa/probar-recolectores.ps1`, que
//! prepara `GD_QA_DIR`, el archivo grande (`GD_QA_ORIGEN`) y la URL de descarga (`GD_QA_URL`).

use super::*;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
    PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

const MB: f64 = 1_048_576.0;
const GB: f64 = 1_073_741_824.0;

struct Pdh {
    consulta: PDH_HQUERY,
    contadores: Vec<PDH_HCOUNTER>,
}

impl Pdh {
    fn nueva(rutas: &[&str]) -> Self {
        let mut consulta = PDH_HQUERY::default();
        assert_eq!(unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut consulta) }, ERROR_SUCCESS.0);
        let contadores = rutas
            .iter()
            .map(|ruta| {
                let w: Vec<u16> = ruta.encode_utf16().chain(std::iter::once(0)).collect();
                let mut c = PDH_HCOUNTER::default();
                let r = unsafe { PdhAddEnglishCounterW(consulta, PCWSTR(w.as_ptr()), 0, &mut c) };
                assert_eq!(r, ERROR_SUCCESS.0, "PDH rechazó {ruta}");
                c
            })
            .collect();
        let p = Self { consulta, contadores };
        p.recolectar();
        p
    }

    fn recolectar(&self) {
        unsafe {
            PdhCollectQueryData(self.consulta);
        }
    }

    fn valor(&self, i: usize) -> f64 {
        let mut v = PDH_FMT_COUNTERVALUE::default();
        if unsafe { PdhGetFormattedCounterValue(self.contadores[i], PDH_FMT_DOUBLE, None, &mut v) } != ERROR_SUCCESS.0 {
            return f64::NAN;
        }
        unsafe { v.Anonymous.doubleValue }
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.consulta);
        }
    }
}

fn dir_qa() -> PathBuf {
    let d = std::env::var_os("GD_QA_DIR").map(PathBuf::from).expect("falta GD_QA_DIR");
    fs::create_dir_all(&d).unwrap();
    d
}

fn guardar(nombre: &str, contenido: &str) {
    fs::write(dir_qa().join(nombre), contenido).unwrap();
}

/// Hilo que sondea una tasa cruda cada `paso` y anota cuándo cruza el umbral por primera vez:
/// separa el retraso del sistema (caché de escritura, conexión TLS) del retraso del recolector.
fn observar<F>(paso: Duration, umbral: f64, crear: F) -> (Arc<AtomicBool>, thread::JoinHandle<Option<Instant>>)
where
    F: FnOnce() -> Box<dyn FnMut() -> f64> + Send + 'static,
{
    let alto = Arc::new(AtomicBool::new(false));
    let a = alto.clone();
    let hilo = thread::spawn(move || {
        let mut tasa = crear();
        tasa();
        while !a.load(Ordering::Relaxed) {
            thread::sleep(paso);
            if tasa() > umbral {
                return Some(Instant::now());
            }
        }
        None
    });
    (alto, hilo)
}

/// Espera al siguiente vencimiento como el planificador de la app y corre la siguiente cita.
fn esperar(proximo: &mut Instant, intervalo: Duration) -> Instant {
    let ahora = Instant::now();
    if *proximo > ahora {
        thread::sleep(*proximo - ahora);
    }
    *proximo += intervalo;
    Instant::now()
}

#[test]
#[ignore = "QA manual (1 min contra PDH): poc/qa/probar-recolectores.ps1"]
fn qa_cpu_y_ram_contra_los_contadores_de_windows() {
    let mut r = Recolector::nuevo();
    let mut h = Historiales::default();
    let pdh = Pdh::nueva(&[r"\Processor(_Total)\% Processor Time", r"\Processor Information(_Total)\% Processor Utility"]);
    let mem = Pdh::nueva(&[r"\Memory\Available Bytes", r"\Memory\Modified Page List Bytes"]);
    r.cpu(&mut h);
    pdh.recolectar();

    let logicos = thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let carga = Arc::new(AtomicBool::new(false));
    let fin = Arc::new(AtomicBool::new(false));
    let hilos: Vec<_> = (0..(logicos / 2).max(1))
        .map(|_| {
            let (carga, fin) = (carga.clone(), fin.clone());
            thread::spawn(move || {
                while !fin.load(Ordering::Relaxed) {
                    if carga.load(Ordering::Relaxed) {
                        let mut x = 0u64;
                        for i in 0..200_000u64 {
                            x = black_box(x.wrapping_add(i));
                        }
                    } else {
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            })
        })
        .collect();

    let intervalo = Duration::from_secs(5);
    let mut proximo = Instant::now() + intervalo;
    let mut lastre: Option<Vec<u8>> = None;
    let mut csv = String::from("muestra,carga,cpu_glassdeck,cpu_processor_time,cpu_processor_utility,ram_usada_glassdeck_gb,ram_total_menos_disponible_gb,ram_en_uso_taskmgr_antes_gb,ram_en_uso_taskmgr_gb,modificada_gb\n");
    let (mut d_tiempo, mut d_util, mut d_ram, mut d_tm, mut max_mod) = (0f64, 0f64, 0f64, 0f64, 0f64);
    for i in 1..=12 {
        if i == 5 {
            carga.store(true, Ordering::Relaxed);
            let m = win::memoria(None);
            let tam = (1536.0 * MB).min(m.total.saturating_sub(m.usada) as f64 / 3.0) as usize;
            lastre = Some(vec![1u8; tam]);
        }
        if i == 9 {
            carga.store(false, Ordering::Relaxed);
            lastre = None;
        }
        esperar(&mut proximo, intervalo);
        let cpu = r.cpu(&mut h).total as f64;
        pdh.recolectar();
        // La RAM cambia entre lecturas (p. ej. al liberar el lastre): se compara contra el
        // intervalo que forman dos lecturas de PDH, una antes y otra después del recolector.
        mem.recolectar();
        let (disp_antes, modif_antes) = (mem.valor(0), mem.valor(1));
        let ram = r.ram(&mut h);
        mem.recolectar();
        let (disp, modif) = (mem.valor(0), mem.valor(1));
        let (tiempo, util) = (pdh.valor(0), pdh.valor(1).min(100.0));
        let total = ram.total as f64;
        let usada = ram.usada as f64 / GB;
        let usada_pdh = (total - disp) / GB;
        let en_uso_tm = (total - disp - modif) / GB;
        let en_uso_antes = (total - disp_antes - modif_antes) / GB;
        let fuera = |v: f64, a: f64, b: f64| if v < a.min(b) { a.min(b) - v } else if v > a.max(b) { v - a.max(b) } else { 0.0 };
        d_tiempo = d_tiempo.max((cpu - tiempo).abs());
        d_util = d_util.max((cpu - util).abs());
        d_ram = d_ram.max(fuera(usada, (total - disp_antes) / GB, usada_pdh));
        d_tm = d_tm.max(fuera(usada, en_uso_antes, en_uso_tm));
        max_mod = max_mod.max(modif / GB);
        let c = if (5..=8).contains(&i) { "si" } else { "no" };
        csv += &format!(
            "{i},{c},{cpu:.1},{tiempo:.1},{util:.1},{usada:.3},{usada_pdh:.3},{en_uso_antes:.3},{en_uso_tm:.3},{:.3}\n",
            modif / GB
        );
    }
    drop(lastre);
    fin.store(true, Ordering::Relaxed);
    for h in hilos {
        let _ = h.join();
    }
    csv += &format!("# max |cpu - processor_time| = {d_tiempo:.2} pts; max |cpu - processor_utility| = {d_util:.2} pts\n");
    csv += &format!("# max distancia de ram al intervalo PDH antes/despues: total - disponible {d_ram:.3} GB; en uso taskmgr {d_tm:.3} GB; max modificada = {max_mod:.3} GB\n");
    println!("{csv}");
    guardar("cpu-ram.csv", &csv);
    assert!(d_util < 5.0, "CPU difiere {d_util:.2} puntos de % Processor Utility (Administrador de tareas)");
    assert!(d_tm < 0.3, "RAM difiere {d_tm:.3} GB de «En uso» del Administrador de tareas");
}

struct MuestraDisco {
    t: Instant,
    escritura: f64,
    lectura: f64,
    pdh_escritura: f64,
    pdh_lectura: f64,
}

impl MuestraDisco {
    fn total(&self) -> f64 {
        self.lectura + self.escritura
    }
    fn pdh_total(&self) -> f64 {
        self.pdh_lectura + self.pdh_escritura
    }
}

const CONTADORES_DISCO: [&str; 2] =
    [r"\PhysicalDisk(_Total)\Disk Write Bytes/sec", r"\PhysicalDisk(_Total)\Disk Read Bytes/sec"];

fn muestra_disco(r: &mut Recolector, h: &mut Historiales, pdh: &Pdh, unidad: &str) -> MuestraDisco {
    let u = r.disco(h);
    pdh.recolectar();
    let (lectura, escritura) =
        u.iter().find(|d| d.unidad == unidad).map(|d| (d.lectura_bps, d.escritura_bps)).unwrap_or((0.0, 0.0));
    MuestraDisco { t: Instant::now(), escritura, lectura, pdh_escritura: pdh.valor(0), pdh_lectura: pdh.valor(1) }
}

#[test]
#[ignore = "QA manual (copia de 4 GB): poc/qa/probar-recolectores.ps1"]
fn qa_disco_unidades_y_copia_grande() {
    let origen = std::env::var_os("GD_QA_ORIGEN").map(PathBuf::from).expect("falta GD_QA_ORIGEN");
    let dir = dir_qa();
    let unidad = origen.to_string_lossy()[..2].to_ascii_uppercase();
    let mut r = Recolector::nuevo();
    let mut h = Historiales::default();
    let unidades = r.disco(&mut h);
    guardar("disco.json", &serde_json::to_string_pretty(&unidades).unwrap());

    let pdh = Pdh::nueva(&CONTADORES_DISCO);
    let intervalo = Duration::from_millis(Intervalos::default().disco_ms);
    let mut proximo = Instant::now() + intervalo;
    esperar(&mut proximo, intervalo);
    muestra_disco(&mut r, &mut h, &pdh, &unidad);
    let mut csv = String::from("prueba,desfase_s,muestra_s,lectura_glassdeck_mbs,escritura_glassdeck_mbs,lectura_physicaldisk_mbs,escritura_physicaldisk_mbs\n");
    let mut resumen = String::new();
    let mut peor = 0f64;
    let bytes = fs::metadata(&origen).unwrap().len();
    for (k, desfase) in [1.0f64, 2.5, 4.0].into_iter().enumerate() {
        let mut previa = muestra_disco(&mut r, &mut h, &pdh, &unidad);
        for _ in 0..8 {
            esperar(&mut proximo, intervalo);
            previa = muestra_disco(&mut r, &mut h, &pdh, &unidad);
            if previa.total() < 10.0 * MB {
                break;
            }
        }
        let umbral = (previa.total() * 3.0).max(10.0 * MB);
        thread::sleep(Duration::from_secs_f64(desfase));
        let (alto, observador) = observar(Duration::from_millis(250), umbral, || {
            let p = Pdh::nueva(&CONTADORES_DISCO);
            Box::new(move || {
                p.recolectar();
                p.valor(0) + p.valor(1)
            })
        });
        let t0 = Instant::now();
        let destino = dir.join(format!("copia-{k}"));
        // Copia sin búfer (como un archivo grande que no está en caché): lee y escribe en el disco desde el inicio.
        let mut copia = Command::new("robocopy")
            .arg(origen.parent().unwrap())
            .arg(&destino)
            .arg(origen.file_name().unwrap())
            .args(["/J", "/NJH", "/NJS", "/NP", "/NFL", "/NDL"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut fin_copia: Option<Instant> = None;
        let mut latencia: Option<f64> = None;
        let mut deteccion: Option<Instant> = None;
        let (mut max_g, mut max_p, mut max_dif_rel) = (0f64, 0f64, 0f64);
        loop {
            let terminado = fin_copia.is_some();
            esperar(&mut proximo, intervalo);
            let m = muestra_disco(&mut r, &mut h, &pdh, &unidad);
            if fin_copia.is_none() && copia.try_wait().ok().flatten().is_some() {
                fin_copia = Some(Instant::now());
            }
            let seg = m.t.duration_since(t0).as_secs_f64();
            if latencia.is_none() && m.total() > umbral {
                latencia = Some(seg);
                deteccion = Some(m.t);
            }
            max_g = max_g.max(m.total());
            max_p = max_p.max(m.pdh_total());
            if m.total() > umbral {
                max_dif_rel = max_dif_rel.max((m.total() - m.pdh_total()).abs() / m.pdh_total().max(1.0));
            }
            csv += &format!(
                "{k},{desfase},{seg:.2},{:.1},{:.1},{:.1},{:.1}\n",
                m.lectura / MB,
                m.escritura / MB,
                m.pdh_lectura / MB,
                m.pdh_escritura / MB
            );
            if terminado || t0.elapsed() > Duration::from_secs(300) {
                break;
            }
        }
        alto.store(true, Ordering::Relaxed);
        let actividad = observador.join().unwrap();
        let copiado = fs::metadata(destino.join(origen.file_name().unwrap())).map(|m| m.len()).unwrap_or(0);
        let _ = fs::remove_dir_all(&destino);
        assert_eq!(copiado, bytes, "robocopy no copió el archivo completo");
        let dur = fin_copia.map(|f| f.duration_since(t0).as_secs_f64()).unwrap_or(f64::NAN);
        let lat = latencia.unwrap_or(f64::INFINITY);
        let inicio_disco = actividad.map(|a| a.duration_since(t0).as_secs_f64()).unwrap_or(f64::NAN);
        let desde_actividad = match (deteccion, actividad) {
            (Some(d), Some(a)) => d.saturating_duration_since(a).as_secs_f64(),
            _ => f64::INFINITY,
        };
        peor = peor.max(lat);
        resumen += &format!(
            "# prueba {k}: desfase {desfase} s, umbral {:.1} MB/s, Windows registra la actividad a los {inicio_disco:.2} s, glassdeck la muestra a los {lat:.2} s desde la copia ({desde_actividad:.2} s desde la actividad real), copia {:.2} GB en {dur:.1} s, pico lectura+escritura glassdeck {:.0} MB/s vs PhysicalDisk {:.0} MB/s, dif. max. por muestra {:.1} %\n",
            umbral / MB,
            bytes as f64 / GB,
            max_g / MB,
            max_p / MB,
            max_dif_rel * 100.0
        );
    }
    csv += &resumen;
    println!("{csv}");
    guardar("disco.csv", &csv);
    assert!(peor < 5.0, "la copia tardó {peor:.2} s en verse (CA < 5 s)");
}

#[test]
#[ignore = "QA manual (descarga de prueba): poc/qa/probar-recolectores.ps1"]
fn qa_red_adaptador_y_descarga() {
    let url = std::env::var("GD_QA_URL").expect("falta GD_QA_URL");
    let dir = dir_qa();
    let mut r = Recolector::nuevo();
    let mut h = Historiales::default();
    let intervalo = Duration::from_millis(Intervalos::default().cpu_ram_red_ms);
    let mut proximo = Instant::now() + intervalo;
    let primera = r.red(&mut h);
    guardar("red.json", &serde_json::to_string_pretty(&primera).unwrap());

    let mut base = 0f64;
    for _ in 0..3 {
        esperar(&mut proximo, intervalo);
        base = base.max(r.red(&mut h).bajada_bps);
    }
    let umbral = (base * 3.0).max(512.0 * 1024.0);
    let mut csv = format!("# linea base max {:.3} MB/s, umbral {:.3} MB/s\nprueba,desfase_s,muestra_s,bajada_mbs,descargando\n", base / MB, umbral / MB);
    let mut resumen = String::new();
    let mut peor = 0f64;
    for (k, desfase) in [0.3f64, 1.0, 1.7].into_iter().enumerate() {
        esperar(&mut proximo, intervalo);
        r.red(&mut h);
        thread::sleep(Duration::from_secs_f64(desfase));
        let destino = dir.join(format!("descarga-{k}.bin"));
        let indice = r.adaptador.as_ref().map(|a| a.indice).expect("sin adaptador activo");
        let (alto, observador) = observar(Duration::from_millis(100), umbral, move || {
            let mut previo: Option<(u64, Instant)> = None;
            Box::new(move || {
                let ahora = Instant::now();
                let entrada = win::bytes_adaptador(indice).map(|b| b.0).unwrap_or(0);
                let tasa = previo
                    .map(|(e, t)| entrada.saturating_sub(e) as f64 / ahora.duration_since(t).as_secs_f64())
                    .unwrap_or(0.0);
                previo = Some((entrada, ahora));
                tasa
            })
        });
        let t0 = Instant::now();
        let mut hijo = Command::new("curl.exe")
            .args(["-s", "-f", "-L", "-A", "Mozilla/5.0", "-o"])
            .arg(&destino)
            .arg(&url)
            .spawn()
            .unwrap();
        let (mut latencia, mut caida, mut fin_descarga): (Option<f64>, Option<f64>, Option<Instant>) = (None, None, None);
        let mut deteccion: Option<Instant> = None;
        let mut pico = 0f64;
        loop {
            esperar(&mut proximo, intervalo);
            let m = r.red(&mut h);
            if fin_descarga.is_none() && hijo.try_wait().ok().flatten().is_some() {
                fin_descarga = Some(Instant::now());
            }
            let seg = t0.elapsed().as_secs_f64();
            if latencia.is_none() && m.bajada_bps > umbral {
                latencia = Some(seg);
                deteccion = Some(Instant::now());
            }
            pico = pico.max(m.bajada_bps);
            csv += &format!("{k},{desfase},{seg:.2},{:.2},{}\n", m.bajada_bps / MB, fin_descarga.is_none());
            if let Some(f) = fin_descarga {
                if m.bajada_bps < umbral {
                    caida = Some(f.elapsed().as_secs_f64());
                    break;
                }
                if f.elapsed() > Duration::from_secs(20) {
                    break;
                }
            }
            if t0.elapsed() > Duration::from_secs(180) {
                let _ = hijo.kill();
            }
        }
        let bytes = fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
        let dur = fin_descarga.map(|f| f.duration_since(t0).as_secs_f64()).unwrap_or(f64::NAN);
        alto.store(true, Ordering::Relaxed);
        let actividad = observador.join().unwrap();
        let _ = fs::remove_file(&destino);
        assert!(bytes > 10_000_000, "la descarga de prueba {k} no bajó datos ({bytes} bytes)");
        let lat = latencia.unwrap_or(f64::INFINITY);
        let primer_trafico = actividad.map(|a| a.duration_since(t0).as_secs_f64()).unwrap_or(f64::NAN);
        let desde_actividad = match (deteccion, actividad) {
            (Some(d), Some(a)) => d.saturating_duration_since(a).as_secs_f64(),
            _ => f64::INFINITY,
        };
        peor = peor.max(lat);
        resumen += &format!(
            "# prueba {k}: desfase {desfase} s, el trafico real empieza a los {primer_trafico:.2} s (conexion TLS), {:.1} MB en {dur:.1} s ({:.1} MB/s promedio curl), pico glassdeck {:.1} MB/s, subida visible a los {lat:.2} s desde curl ({desde_actividad:.2} s desde el trafico real), caida visible {:.2} s despues de terminar\n",
            bytes as f64 / MB,
            bytes as f64 / MB / dur,
            pico / MB,
            caida.unwrap_or(f64::NAN)
        );
    }
    csv += &resumen;
    println!("{csv}");
    guardar("red.csv", &csv);
    assert!(peor < 2.0, "la descarga tardó {peor:.2} s en verse (CA < 2 s)");
}
