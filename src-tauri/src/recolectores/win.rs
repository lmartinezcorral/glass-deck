use std::collections::HashMap;
use std::mem::size_of;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GetIfEntry2, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST,
    GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH, MIB_IF_ROW2,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, AF_UNSPEC, SOCKADDR_IN};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};

pub fn con_bateria() -> bool {
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.ACLineStatus == 0 }
}

pub fn pantalla_completa_ocupada() -> bool {
    match unsafe { SHQueryUserNotificationState() } {
        Ok(estado) => estado == QUNS_BUSY || estado == QUNS_RUNNING_D3D_FULL_SCREEN || estado == QUNS_PRESENTATION_MODE,
        Err(_) => false,
    }
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct Memoria {
    pub total: u64,
    pub usada: u64,
    pub en_cache: u64,
    pub libre: u64,
    pub comprometida: u64,
    pub limite_comprometida: u64,
}

pub fn memoria() -> Memoria {
    let mut ms = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    let mut pi = PERFORMANCE_INFORMATION { cb: size_of::<PERFORMANCE_INFORMATION>() as u32, ..Default::default() };
    unsafe {
        let _ = GlobalMemoryStatusEx(&mut ms);
        let _ = GetPerformanceInfo(&mut pi, size_of::<PERFORMANCE_INFORMATION>() as u32);
    }
    let pagina = pi.PageSize as u64;
    let total = ms.ullTotalPhys;
    let disponible = ms.ullAvailPhys;
    let en_cache = (pi.SystemCache as u64 * pagina).min(disponible);
    Memoria {
        total,
        usada: total.saturating_sub(disponible),
        en_cache,
        libre: disponible.saturating_sub(en_cache),
        comprometida: pi.CommitTotal as u64 * pagina,
        limite_comprometida: pi.CommitLimit as u64 * pagina,
    }
}

pub struct ConsultaDisco {
    consulta: PDH_HQUERY,
    lectura: PDH_HCOUNTER,
    escritura: PDH_HCOUNTER,
    lista: bool,
}

unsafe impl Send for ConsultaDisco {}

fn amplio(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl ConsultaDisco {
    pub fn nueva() -> Option<Self> {
        unsafe {
            let mut consulta = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut consulta) != ERROR_SUCCESS.0 {
                return None;
            }
            let mut lectura = PDH_HCOUNTER::default();
            let mut escritura = PDH_HCOUNTER::default();
            let l = amplio(r"\LogicalDisk(*)\Disk Read Bytes/sec");
            let e = amplio(r"\LogicalDisk(*)\Disk Write Bytes/sec");
            if PdhAddEnglishCounterW(consulta, PCWSTR(l.as_ptr()), 0, &mut lectura) != ERROR_SUCCESS.0
                || PdhAddEnglishCounterW(consulta, PCWSTR(e.as_ptr()), 0, &mut escritura) != ERROR_SUCCESS.0
            {
                return None;
            }
            PdhCollectQueryData(consulta);
            Some(Self { consulta, lectura, escritura, lista: false })
        }
    }

    pub fn muestrear(&mut self) -> HashMap<String, (f64, f64)> {
        let mut salida = HashMap::new();
        unsafe {
            if PdhCollectQueryData(self.consulta) != ERROR_SUCCESS.0 {
                return salida;
            }
        }
        if !self.lista {
            self.lista = true;
            return salida;
        }
        let lecturas = leer_arreglo(self.lectura);
        let escrituras = leer_arreglo(self.escritura);
        for (unidad, l) in lecturas {
            let e = escrituras.get(&unidad).copied().unwrap_or(0.0);
            salida.insert(unidad, (l, e));
        }
        salida
    }
}

fn leer_arreglo(contador: PDH_HCOUNTER) -> HashMap<String, f64> {
    let mut salida = HashMap::new();
    unsafe {
        let mut bytes: u32 = 0;
        let mut cuenta: u32 = 0;
        let r = PdhGetFormattedCounterArrayW(contador, PDH_FMT_DOUBLE, &mut bytes, &mut cuenta, None);
        if r != PDH_MORE_DATA || bytes == 0 {
            return salida;
        }
        let mut buf = vec![0u8; bytes as usize];
        let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        if PdhGetFormattedCounterArrayW(contador, PDH_FMT_DOUBLE, &mut bytes, &mut cuenta, Some(items)) != ERROR_SUCCESS.0 {
            return salida;
        }
        for i in 0..cuenta as usize {
            let item = &*items.add(i);
            let nombre = pwstr_a_string(item.szName);
            if nombre.len() == 2 && nombre.ends_with(':') {
                salida.insert(nombre.to_ascii_uppercase(), item.FmtValue.Anonymous.doubleValue);
            }
        }
    }
    salida
}

fn pwstr_a_string(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { p.to_string().unwrap_or_default() }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Adaptador {
    pub nombre: String,
    pub tipo: String,
    pub ip: Option<String>,
    #[serde(skip)]
    pub indice: u32,
    #[serde(skip)]
    pub metrica: u32,
}

pub fn adaptador_activo() -> Option<Adaptador> {
    let mut tam: u32 = 16 * 1024;
    let mut buf: Vec<u8>;
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    loop {
        buf = vec![0u8; tam as usize];
        let r = unsafe {
            GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut tam)
        };
        if r == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if r != ERROR_SUCCESS.0 {
            return None;
        }
        break;
    }
    let mut mejor: Option<Adaptador> = None;
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        let a = unsafe { &*p };
        p = a.Next;
        if a.OperStatus != IfOperStatusUp || a.FirstGatewayAddress.is_null() {
            continue;
        }
        let tipo = match a.IfType {
            71 => "Wi-Fi",
            6 => "Ethernet",
            243 | 244 => "Celular",
            _ => "Otro",
        };
        let ip = ipv4_de(a);
        let candidato = Adaptador {
            nombre: pwstr_a_string(a.FriendlyName),
            tipo: tipo.to_string(),
            ip,
            indice: unsafe { a.Anonymous1.Anonymous.IfIndex },
            metrica: a.Ipv4Metric,
        };
        if mejor.as_ref().map(|m| candidato.metrica < m.metrica).unwrap_or(true) {
            mejor = Some(candidato);
        }
    }
    mejor
}

fn ipv4_de(a: &IP_ADAPTER_ADDRESSES_LH) -> Option<String> {
    let mut u = a.FirstUnicastAddress;
    while !u.is_null() {
        let dir = unsafe { &*u };
        let sa = dir.Address.lpSockaddr;
        if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
            let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
            let b = unsafe { sin.sin_addr.S_un.S_un_b };
            return Some(format!("{}.{}.{}.{}", b.s_b1, b.s_b2, b.s_b3, b.s_b4));
        }
        u = dir.Next;
    }
    None
}

pub fn bytes_adaptador(indice: u32) -> Option<(u64, u64)> {
    let mut fila = MIB_IF_ROW2 { InterfaceIndex: indice, ..Default::default() };
    let r = unsafe { GetIfEntry2(&mut fila) };
    if r.is_err() {
        return None;
    }
    Some((fila.InOctets, fila.OutOctets))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn en_el_equipo_real_el_desglose_de_ram_suma_el_total() {
        let m = memoria();
        assert!(m.total > 0);
        let suma = m.usada + m.en_cache + m.libre;
        assert!(suma.abs_diff(m.total) as f64 / m.total as f64 <= 0.01);
        assert!(m.usada <= m.total);
    }

    #[test]
    fn en_el_equipo_real_hay_al_menos_una_lectura_de_red_o_ninguna_conexion() {
        if let Some(a) = adaptador_activo() {
            assert!(!a.nombre.is_empty());
            assert!(bytes_adaptador(a.indice).is_some());
        }
    }
}
