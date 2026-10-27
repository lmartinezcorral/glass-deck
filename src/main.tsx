import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './tema/tokens.css';
import './tema/base.css';
import './componentes/componentes.css';
import './vistas/vistas.css';
import { App } from './App';

const raiz = document.getElementById('raiz');
if (raiz) {
  createRoot(raiz).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
