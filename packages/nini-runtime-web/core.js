// El motor de reactividad de Nini
export const nini = {
    currentEffect: null,
    
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
    
    updateDOM() {
        console.log("Nini detectó un cambio y actualizando UI...");
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
            const module = await import(route.js);
            console.log('Cargado:', route.page);
        } catch (err) {
            console.error('Error al cargar página:', err);
        }
    }
}

// Auto-inicializar cuando el DOM esté listo
if (typeof window !== 'undefined') {
    window.addEventListener('DOMContentLoaded', () => {
        if (!window.niniRouter) {
            window.niniRouter = new NiniRouter();
        }
    });
}

export { NiniRouter };
