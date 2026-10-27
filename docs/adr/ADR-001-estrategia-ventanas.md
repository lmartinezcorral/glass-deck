# ADR-001 — Estrategia de ventanas de los widgets

| Campo | Valor |
|---|---|
| Estado | Propuesto (se llena con la prueba de concepto de la fase 0) |
| Fecha | 2026-10-27 |
| Riesgo | R-01 (`SPEC.md` §7) |
| Requerimiento | GD-NFR-PERF-02: RAM < 200 MB en total (proceso principal + WebView2) |

## Contexto

Cada widget es una ventana flotante independiente (D-02). En Tauri 2, cada ventana es un WebView2, y WebView2 puede crear procesos de render por ventana. Con 11 ventanas (CPU, RAM, Disco, Red, Procesos, Spending, Correo/Agenda, Clima, Audio, Brillo, Atajos) más el dock, el consumo puede pasar del presupuesto.

## Variantes medidas

| Variante | Descripción |
|---|---|
| A | 11 ventanas, entorno de WebView2 compartido (comportamiento por defecto de Tauri) |
| B | Igual que A, pero las ventanas de widgets cerrados se destruyen en lugar de ocultarse |
| C | Una ventana transparente por monitor que dibuja varios widgets, con zonas de clic |

## Resultados

Pendiente.

## Decisión

Pendiente.

## Consecuencias

Pendiente.
