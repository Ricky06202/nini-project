// El motor de reactividad de Nini
export const nini = {
    currentEffect: null,
    // Crea un valor que "avisa" cuando cambia
    signal(initialValue) {
        let value = initialValue;
        const subscribers = new Set();
        return {
            get value() {
                // Registrar el effect actual si existe
                if (nini.currentEffect) {
                    subscribers.add(nini.currentEffect);
                }
                return value;
            },
            set value(newValue) {
                if (value !== newValue) {
                    value = newValue;
                    // Ejecutar todos los suscriptores (effects)
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
        // Si hay una instancia activa, buscar solo en esa
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
    // Crea un effect que se re-ejecuta cuando cambian las signals a las que accede
    effect(fn) {
        const effectFn = () => {
            nini.currentEffect = effectFn;
            fn();
            nini.currentEffect = null;
        };
        // Ejecutar inmediatamente para suscribirse a las signals
        effectFn();
    },
    updateDOM() {
        console.log("Nini detectó un cambio y está actualizando la UI...");
    }
};
