package fixtures;

public final class Dep {

    private static final StringBuilder LOG = new StringBuilder();

    private Dep() {
    }

    public static void reset() {
        LOG.setLength(0);
    }

    public static String[] events() {
        String[] out = new String[LOG.length()];
        for (int i = 0; i < out.length; i++) {
            out[i] = String.valueOf(LOG.charAt(i));
        }
        return out;
    }

    public static boolean a() {
        LOG.append('A');
        return false;
    }

    public static boolean b() {
        LOG.append('B');
        return true;
    }
}
