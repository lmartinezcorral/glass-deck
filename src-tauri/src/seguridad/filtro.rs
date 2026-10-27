const MARCA: &str = "[oculto]";
const LARGO_MIN_SECRETO: usize = 24;

pub fn enmascarar(texto: &str) -> String {
    let sin_rutas = enmascarar_rutas_usuario(texto);
    sin_rutas
        .split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';')
        .map(enmascarar_fragmento)
        .collect()
}

fn enmascarar_fragmento(fragmento: &str) -> String {
    let fin = fragmento
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_whitespace() || *c == '"' || *c == '\'' || *c == ',' || *c == ';'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let (palabra, resto) = fragmento.split_at(fin);
    if palabra.is_empty() {
        return fragmento.to_string();
    }
    if let Some((clave, _)) = palabra.split_once('=') {
        let clave_min = clave.to_ascii_lowercase();
        if ["token", "cookie", "secret", "secreto", "password", "clave", "key", "authorization", "session"]
            .iter()
            .any(|k| clave_min.contains(k))
        {
            return format!("{clave}={MARCA}{resto}");
        }
    }
    if parece_correo(palabra) || parece_secreto(palabra) {
        return format!("{MARCA}{resto}");
    }
    fragmento.to_string()
}

fn parece_correo(p: &str) -> bool {
    match p.split_once('@') {
        Some((a, b)) => !a.is_empty() && b.contains('.') && !b.starts_with('.') && !b.ends_with('.'),
        None => false,
    }
}

fn parece_secreto(p: &str) -> bool {
    if p.len() < LARGO_MIN_SECRETO {
        return false;
    }
    let validos = p
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '/' | '='));
    let letras = p.chars().any(|c| c.is_ascii_alphabetic());
    let digitos = p.chars().any(|c| c.is_ascii_digit());
    validos && letras && digitos && !p.contains("::")
}

fn enmascarar_rutas_usuario(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let bajo = texto.to_ascii_lowercase();
    let mut i = 0;
    while i < texto.len() {
        let resto = &bajo[i..];
        let prefijo = ["c:\\users\\", "c:/users/"].iter().find(|p| resto.starts_with(*p));
        match prefijo {
            Some(p) => {
                salida.push_str(&texto[i..i + p.len()]);
                i += p.len();
                let fin = texto[i..]
                    .find(['\\', '/', ' ', '"', '\''])
                    .map(|n| i + n)
                    .unwrap_or(texto.len());
                salida.push_str("<usuario>");
                i = fin;
            }
            None => {
                let c = texto[i..].chars().next().unwrap_or(' ');
                salida.push(c);
                i += c.len_utf8();
            }
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn oculta_el_nombre_de_usuario_en_rutas() {
        let r = enmascarar(r"No se pudo leer C:\Users\hulkmtz\AppData\Roaming\Glassdeck\settings.json");
        assert!(!r.contains("hulkmtz"));
        assert!(r.contains(r"C:\Users\<usuario>\AppData"));
    }

    #[test]
    fn oculta_valores_de_claves_sensibles() {
        let r = enmascarar("cookie=abc123 token=xyz session_id=1");
        assert_eq!(r, "cookie=[oculto] token=[oculto] session_id=[oculto]");
    }

    #[test]
    fn oculta_correos_y_cadenas_tipo_secreto() {
        let r = enmascarar("de persona@ejemplo.com con eyJhbGciOiJIUzI1NiJ9abc123def456");
        assert!(!r.contains("persona@ejemplo.com"));
        assert!(!r.contains("eyJhbGciOiJIUzI1NiJ9abc123def456"));
    }

    #[test]
    fn deja_intacto_el_texto_normal() {
        let t = "Widget cpu creado en monitor 2 (1920x1080 @ 125 %)";
        assert_eq!(enmascarar(t), t);
    }
}
