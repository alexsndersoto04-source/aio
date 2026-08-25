import React from 'react';
import ReactDOM from 'react-dom/client';
import { AuthProvider } from './auth.jsx';
import { ThemeProvider } from './theme.jsx';
import App from './App.jsx';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <ThemeProvider>
      <AuthProvider>
        <App />
      </AuthProvider>
    </ThemeProvider>
  </React.StrictMode>
);
