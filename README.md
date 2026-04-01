# Nini Framework

**Un framework todo-en-uno para construir aplicaciones modernas**

Nini es un framework creado desde cero en Rust que te permite construir aplicaciones para web, escritorio y móvil con una única sintaxis. Diseñado para desarrolladores que quieren potencia sin complejidad.

## ¿Qué es Nini?

Nini es un framework full-stack que combina:
- **Compilador en Rust** - Rendimiento extremo y reportes de error claros
- **Runtime en JavaScript** - Compatibilidad universal con navegadores
- **Sintaxis Blazor** - Directivas limpias y legibles (`@if`, `@foreach`)
- **Reactividad integrada** - Signals como Svelte pero sin build complex
- **Scoped CSS** - Estilos que no se fugan

## Características Principales

### Reactividad con Signals

```nini
<script>
    contador = 0
    nombre = "Mundo"
</script>

<div>
    <h1>Hola {nombre}!</h1>
    <p>Has hecho clic {contador} veces</p>
    <button onclick="contador++">+1</button>
    <button onclick="contador = 0">Reset</button>
</div>
```

### Directivas Blazor

```nini
@* Condicional *@
@if (usuario.logueado) {
    <p>Bienvenido, {usuario.nombre}</p>
}

@* Bucle *@
@foreach (var producto in productos) {
    <div class="card">
        <h3>{producto.nombre}</h3>
        <p>${producto.precio}</p>
    </div>
}
```

### Layouts y Slots

```nini
<!-- components/Layout.nini -->
<header>...</header>
<main>
    <slot />
</main>
<footer>...</footer>
```

```nini
<!-- pages/index.nini -->
<script>
    import "../components/Layout.nini" as Layout
</script>

<Layout>
    <h1>Este contenido va en el slot</h1>
</Layout>
```

### Servicios e Inyección

```nini
<script>
    service ApiService
        baseUrl = "https://api.example.com"
        
        fn obtener_usuarios()
            return fetch(this.baseUrl + "/users")
    
    api = inject ApiService
    usuarios = []
    
    fn init()
        usuarios = api.obtener_usuarios()
</script>
```

### Scoped CSS

```nini
<style>
    container:
        max-width: 1200px
        margin: 0 auto
        padding: 2rem
    
    title:
        color: gold
        font-size: 2.5rem
    
    button:
        background: #252525
        color: gold
        border: 1px solid gold
        padding: 0.5rem 1rem
        border-radius: 4px
        cursor: pointer
</style>
```

Los estilos solo aplican al componente actual - no hay fugas CSS.

### SPA Router Integrado

```nini
<!-- Los Links se transforman automáticamente -->
<Link to="/">Inicio</Link>
<Link to="/perfil">Perfil</Link>
<Link to="/acerca">Acerca</Link>
```

## Plataformas Soportadas

| Plataforma | Estado | Descripción |
|------------|--------|-------------|
| Web | ✅ Soportado | SPA completa con router integrado |
| Escritorio | 🔜 Próximamente | Via Tauri/Electron |
| Android | 🔜 Próximamente | Via WebView nativo |
| iOS | ❌ No soportado | Apple no permite engines alternativas |
| macOS | ❌ No soportado | Limitaciones de Apple |

## Instalación

### Requisitos

```bash
# Rust (última versión stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Bun (gestor de paquetes)
curl -fsSL https://bun.sh/install | bash
```

### Crear un nuevo proyecto

```bash
# Instalar Nini CLI
cargo install nini-cli

# Crear proyecto
nini init mi-proyecto

# Entrar al proyecto
cd mi-proyecto
```

### Estructura del proyecto

```
mi-proyecto/
├── src/
│   ├── pages/
│   │   ├── index.nini        # Página /
│   │   ├── perfil.nini       # Página /perfil
│   │   └── about.nini        # Página /about
│   ├── components/
│   │   ├── Header.nini       # Componente Header
│   │   └── Card.nini         # Componente Card
│   ├── services/
│   │   └── api.nini          # Servicio API
│   └── styles/
│       └── global.css        # Estilos globales
├── nini.config.js
└── package.json
```

### Comandos

```bash
nini dev          # Servidor de desarrollo con hot reload
nini build        # Build de producción
nini preview      # Preview del build
```

## Sintaxis Completa

### Variables y Reactividad

```nini
<script>
    // Variables reactivas (se actualizan la UI automáticamente)
    contador = 0
    nombre = "Nini"
    activo = true
    items = ["uno", "dos", "tres"]
    
    // Acceso a valores
    // contador      → Lectura
    // contador++    → Incremento
    // contador = 5  → Asignación
</script>
```

### Directivas de Control

```nini
@* If simple *@
@if (activo) {
    <p>Elemento visible</p>
}

@* If-else *@
@if (usuario.logueado) {
    <p>Bienvenido</p>
}

@* Foreach *@
@foreach (var item in items) {
    <li>{item}</li>
}
```

### Componentes

```nini
<!-- Definir: components/Alert.nini -->
<script>
    prop message = "Alerta"
    prop type = "info"
</script>

<div class="alert {type}">
    <p>{message}</p>
</div>

<style>
    alert:
        padding: 1rem
        border-radius: 8px
    info:
        background: blue
        color: white
    warning:
        background: orange
        color: black
</style>
```

```nini
<!-- Usar en cualquier página -->
<script>
    import "../components/Alert.nini" as Alert
</script>

<Alert message="Hola mundo" type="info" />
```

### Servicios

```nini
<!-- services/auth.service.nini -->
service AuthService
    apiUrl = "https://api.miapp.com"
    token = ""
    
    fn login(email, password)
        return fetch(this.apiUrl + "/login", {
            method: "POST",
            body: JSON.stringify({email, password})
        }).then(r => r.json())
    
    fn logout()
        this.token = ""
        window.location = "/login"
```

```nini
<!-- Uso en componente -->
<script>
    import "../services/auth.service.nini" as AuthService
    
    auth = inject AuthService
    email = ""
    password = ""
    
    fn handleLogin()
        result = auth.login(email, password)
        auth.token = result.token
</script>
```

## Ciclo de Vida

```nini
<script>
    datos = []
    
    fn init()
        // Se ejecuta cuando el componente se monta
        // Perfecto para cargar datos iniciales
        datos = await api.obtener_datos()
</script>
```

## Roadmap

### Fase 9: State Management
- [ ] Stores globales (Zustand style)
- [ ] Persistencia automática (localStorage)
- [ ] DevTools para debug de signals

### Fase 10: Formularios
- [ ] Binding bidireccional `bind:value`
- [ ] Validación integrada `@valid`
- [ ] Form submission automático

### Fase 11: Animaciones
- [ ] Directiva `@animate`
- [ ] Transiciones de página
- [ ] Spring animations

### Fase 12: TypeScript
- [ ] Tipado estático para servicios
- [ ] Generación de tipos desde schemas
- [ ] VSCode extension

### Fase 13: Build Optimizations
- [ ] Code splitting automático
- [ ] Tree shaking
- [ ] Lazy loading de rutas
- [ ] Compresión Brot/Gzip

### Fase 14: Desktop (Tauri)
- [ ] Integración con Tauri
- [ ] APIs nativas (file system, notifications)
- [ ] Auto-update

### Fase 15: Mobile (Android)
- [ ] WebView wrapper nativo
- [ ] APIs nativas (camera, GPS, etc.)
- [ ] Push notifications

### Fase 16: SSR y SSG
- [ ] Server-Side Rendering
- [ ] Static Site Generation
- [ ] Incremental regeneration

### Fase 17: Testing
- [ ] Testing utils integrados
- [ ] Snapshot testing
- [ ] E2E testing helpers

### Fase 18: Component Library
- [ ] UI Kit oficial (buttons, forms, modals)
- [ ] Temas pre-configurados
- [ ] Accessible components

## Comparación con Otros Frameworks

| Feature | Nini | Svelte | React | Angular | Blazor |
|---------|------|--------|-------|---------|--------|
| Sintaxis | Blazor-like | Template | JSX | TypeScript | Razor |
| Tamaño bundle | ~15kb | ~20kb | ~45kb | ~140kb | ~150kb* |
| Reactividad | Signals | Stores | Hooks | RxJS | .NET |
| Scoped CSS | ✅ | ✅ | ❌ | ✅ | ❌ |
| SSR | 🔜 | ✅ | ✅ | ✅ | ✅ |
| Mobile | 🔜 | ❌ | ❌ | ❌ | ✅* |

*Blazor WebAssembly requiere download del runtime .NET

## Ejemplos

### Contador
```nini
<script>
    count = 0
</script>

<button onclick="count++">Clicks: {count}</button>
```

### Lista de Tareas
```nini
<script>
    tareas = []
    nueva_tarea = ""
    
    fn agregar()
        if (nueva_tarea != "") {
            tareas = [...tareas, nueva_tarea]
            nueva_tarea = ""
        }
    
    fn eliminar(index)
        tareas = tareas.filter((_, i) => i != index)
</script>

<div>
    <input bind:value={nueva_tarea} placeholder="Nueva tarea..." />
    <button onclick="agregar()">Agregar</button>
    
    @foreach (var tarea in tareas) {
        <div class="tarea">
            <span>{tarea}</span>
            <button onclick="eliminar({index})">×</button>
        </div>
    }
</div>
```

### Login
```nini
<script>
    service AuthService
        login(email, password)
            return fetch("/api/login", {
                method: "POST",
                body: JSON.stringify({email, password})
            })
    
    auth = inject AuthService
    email = ""
    password = ""
    error = ""
    
    fn submit()
        result = auth.login(email, password)
        if (result.error)
            error = result.error
        else
            window.location = "/dashboard"
</script>

<form onsubmit="submit()">
    <input type="email" bind:value={email} placeholder="Email" />
    <input type="password" bind:value={password} placeholder="Password" />
    @if (error) {
        <p class="error">{error}</p>
    }
    <button type="submit">Ingresar</button>
</form>
```

## Contribuir

Nini es open source y acepta contribuciones:

```bash
git clone https://github.com/tu-usuario/nini
cd nini
cargo build
cargo test
```

## Licencia

MIT License - Usa libremente en proyectos personales y comerciales.

---

**Nini** - Framework todo-en-uno para la web moderna 🦀
