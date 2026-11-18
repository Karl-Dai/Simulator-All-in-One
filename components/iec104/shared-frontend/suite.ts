/** The suite owns updates; standalone builds keep their existing updater. */
export const isSimLabSuite = import.meta.env.VITE_SIMLAB_SUITE === '1'
