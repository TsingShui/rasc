package driver;

import fixtures.Case;
import fixtures.Dep;

public final class Driver {

    private static final String CASE = "D006";

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: Driver <invocation-id>");
            System.exit(2);
            return;
        }
        String invocation = args[0];
        String result = null;
        boolean completed = false;
        Dep.reset();
        if ("v1".equals(invocation)) {
            result = booleanResult(Case.and());
            completed = true;
        } else if ("v2".equals(invocation)) {
            result = booleanResult(Case.or());
            completed = true;
        } else {
            System.err.println("unknown invocation: " + invocation);
            System.exit(2);
            return;
        }
        System.out.println(record(invocation, result, eventArray(), null, "{}", completed));
    }

    private static String booleanResult(boolean value) {
        return "{\"kind\":\"boolean\",\"value\":" + value + "}";
    }

    private static String eventArray() {
        String[] events = Dep.events();
        StringBuilder out = new StringBuilder("[");
        for (int i = 0; i < events.length; i++) {
            if (i > 0) {
                out.append(',');
            }
            out.append("{\"seq\":").append(i).append(",\"kind\":\"call\",\"label\":")
                    .append(quote(events[i])).append('}');
        }
        return out.append(']').toString();
    }

    private static String record(String invocation, String result, String events, String exception,
            String heap, boolean completed) {
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"").append(CASE).append("\",\"invocation\":").append(quote(invocation));
        out.append(",\"result\":").append(result == null ? "null" : result);
        out.append(",\"events\":").append(events);
        out.append(",\"exception\":").append(exception == null ? "null" : quote(exception));
        out.append(",\"heap\":").append(heap);
        out.append(",\"completed\":").append(completed);
        return out.append('}').toString();
    }

    private static String quote(String value) {
        StringBuilder out = new StringBuilder();
        out.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '"' || c == '\\') {
                out.append('\\').append(c);
            } else if (c < 0x20) {
                out.append(String.format("\\u%04x", (int) c));
            } else {
                out.append(c);
            }
        }
        return out.append('"').toString();
    }
}
