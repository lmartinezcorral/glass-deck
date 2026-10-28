const COMANDOS: &[&str] = &[
    "config_leer",
    "config_guardar",
    "widget_activar",
    "widget_estado",
    "widget_maximizar",
    "widget_fijar",
    "widget_cerrar",
    "historial",
    "procesos_cerrar",
    "proceso_es_fijo",
    "procesos_fijos",
    "limpieza_vista_previa",
    "limpieza_ejecutar",
    "panel_abrir",
    "panel_cerrar",
    "atajos_estado",
    "atajo_cambiar",
    "privacidad_alternar",
    "mostrar_ocultar_todo",
    "restablecer_posiciones",
    "reiniciar_explorer",
    "abrir_administrador_tareas",
    "estado_entorno",
    "boveda_estado",
    "boveda_guardar",
    "boveda_borrar",
    "servicio_ultimo",
    "catalogo_widgets",
    "clima_buscar_ciudad",
    "clima_fijar_ubicacion",
    "avisos_probar",
    "avisos_historial",
    "google_cliente_importar",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMANDOS)),
    )
    .expect("no se pudo preparar la compilación de Tauri");
}
