// El motor de reactividad de Nini
export const nini = {
    currentEffect: null,
    stores: {},
    
    // log() - wrapper para console.log
    log(...args) {
        console.log(...args);
    },
    
    signal(initialValue) {
        let value = initialValue;
        const subscribers = new Set();
        return {
            get value() {
                if (nini.currentEffect) {
                    subscribers.add(nini.currentEffect);
                }
                return value;
            },
            set value(newValue) {
                if (value !== newValue) {
                    value = newValue;
                    subscribers.forEach(subscriber => subscriber());
                }
            },
            subscribe(fn) {
                subscribers.add(fn);
                return () => subscribers.delete(fn);
            }
        };
    },
    
    // Phase 11: Computed signals (derived values)
    computed(computeFn) {
        const signal = nini.signal(computeFn());
        nini.effect(() => {
            signal.value = computeFn();
        });
        return signal;
    },
    
    _currentInstance: null,
    
    setCurrentInstance(instance) {
        nini._currentInstance = instance;
    },
    
    prop(name, defaultValue) {
        if (nini._currentInstance) {
            const comp = document.querySelector(`.comp[data-instance="${nini._currentInstance}"]`);
            if (comp) {
                const attr = `data-${name}-${nini._currentInstance}`;
                if (comp.hasAttribute(attr)) {
                    return comp.getAttribute(attr);
                }
            }
        }
        return defaultValue;
    },
    
    effect(fn) {
        const effectFn = () => {
            nini.currentEffect = effectFn;
            fn();
            nini.currentEffect = null;
        };
        effectFn();
    },
    
    // Phase 14: onChange - like Svelte afterUpdate
    // onChange([var1, var2], () => { ... })
    // Se ejecuta cuando CUALQUIERA de las variables cambia
    onChange(variables, effectFn) {
        const vars = Array.isArray(variables) ? variables : [variables];
        const unsubscribes = vars.map(varName => {
            const signal = window['_' + varName];
            if (signal) {
                return signal.subscribe(() => effectFn());
            }
        }).filter(u => u);
        return () => unsubscribes.forEach(u => u());
    },

    // Legacy alias
    useEffect(variables, effectFn) {
        return nini.onChange(variables, effectFn);
    },

    // Phase 11: Lifecycle - onMount and onUnmount
    _mountCallbacks: [],
    _unmountCallbacks: [],
    
    onMount(fn) {
        this._mountCallbacks.push(fn);
    },
    
    onUnmount(fn) {
        this._unmountCallbacks.push(fn);
    },
    
    onDestroy(fn) {
        this._unmountCallbacks.push(fn);
    },
    
    triggerMount() {
        this._mountCallbacks.forEach(fn => fn());
    },
    
    triggerUnmount() {
        this._unmountCallbacks.forEach(fn => fn());
        this._mountCallbacks = [];
        this._unmountCallbacks = [];
    },
    
    updateDOM() {
        console.log("Nini detectó un cambio y actualizando UI...");
    },
    
    bindHandler() {
        document.querySelectorAll('[data-nini-bind]').forEach(el => {
            const varName = el.getAttribute('data-nini-bind');
            const internalVar = '_' + varName;
            
            // Set initial value from signal if exists
            if (window[internalVar]) {
                el.value = window[internalVar].value;
            }
            
            // Phase 12: onChange handler - listen for input changes (both input and change events)
            const updateValue = (e) => {
                const newValue = e.target.value;
                if (window[internalVar]) {
                    window[internalVar].value = newValue;
                } else {
                    window[internalVar] = nini.signal(newValue);
                }
                window[varName] = newValue;
            };
            
            el.addEventListener('input', updateValue);
            el.addEventListener('change', updateValue);
        });
    },
    
    // Phase 13: Await/async support
    async: (promiseOrFn) => {
        if (typeof promiseOrFn === 'function') {
            return new Promise((resolve, reject) => {
                try {
                    const result = promiseOrFn();
                    if (result instanceof Promise) {
                        result.then(resolve).catch(reject);
                    } else {
                        resolve(result);
                    }
                } catch (e) {
                    reject(e);
                }
            });
        }
        return promiseOrFn;
    }
};

// Router de Nini - intercepta clics y maneja historial
class NiniRouter {
    constructor() {
        this.currentRoute = null;
        this.init();
    }
    
    init() {
        document.addEventListener('click', (e) => {
            const link = e.target.closest('[data-nini-link]');
            if (link) {
                e.preventDefault();
                const href = link.getAttribute('href');
                this.navigate(href);
            }
        });
        
        window.addEventListener('popstate', () => {
            this.loadRoute(window.location.pathname);
        });
        
        this.loadRoute(window.location.pathname);
    }
    
    async navigate(path) {
        window.history.pushState({}, '', path);
        await this.loadRoute(path);
    }
    
    async loadRoute(path) {
        const route = window.NINI_ROUTES?.[path];
        
        if (!route) {
            console.warn('Ruta no encontrada:', path);
            return;
        }
        
        this.currentRoute = path;
        
        try {
            if (route.render) {
                route.render();
            } else if (route.js) {
                await import(route.js);
            }
            console.log('Cargado:', route.page);
        } catch (err) {
            console.error('Error al cargar página:', err);
        }
    }
}

// Auto-inicializar cuando el DOM esté listo
if (typeof window !== 'undefined') {
    // Ensure window.nini exists before any page code runs
    window.nini = window.nini || { stores: {}, currentEffect: null };
    
    window.addEventListener('DOMContentLoaded', () => {
        if (!window.niniRouter) {
            window.niniRouter = new NiniRouter();
        }
    });
}

export { NiniRouter };
