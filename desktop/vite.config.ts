import { defineConfig } from 'vite';
export default defineConfig({clearScreen:false,server:{port:1420,strictPort:true},worker:{format:'es'},build:{target:'es2022',chunkSizeWarningLimit:700}});
