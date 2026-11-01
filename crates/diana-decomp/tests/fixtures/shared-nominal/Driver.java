public class Driver {
    static String value(Object value, fixtures.Box b, fixtures.Box other, fixtures.Token t) {
        return value == null ? "null" : value == b ? "b" : value == other ? "other" : value == t ? "t" : "unexpected";
    }
    static String state(Object[] a, fixtures.Box b, fixtures.Token t) {
        if (a == null) return "null";
        StringBuilder s = new StringBuilder("[");
        for (int i = 0; i < a.length; i++) { if (i != 0) s.append(','); s.append(value(a[i], b, null, t)); }
        return s.append(']').toString();
    }
    public static void main(String[] args) {
        for (boolean nil : new boolean[]{false,true}) {
            fixtures.Box b = nil ? null : new fixtures.Box();
            fixtures.Box other = new fixtures.Box();
            fixtures.Token t = new fixtures.Token();
            for (int flag : new int[]{0,1})
                System.out.println("pick:"+nil+":"+flag+":"+value(fixtures.Case.pick(b,other,flag), b,other,t));
            for (boolean nullToken : new boolean[]{false,true}) for (int flag : new int[]{0,1})
                System.out.println("tags:"+nil+":"+nullToken+":"+flag+":"+fixtures.Case.tags(b,nullToken?null:t,flag));
            for (String op : new String[]{"store","repeat"}) for (String shape : new String[]{"objects","short","boxes","tokens","null"}) {
                Object[] a = shape.equals("null") ? null : shape.equals("short") ? new Object[1]
                    : shape.equals("boxes") ? new fixtures.Box[3] : shape.equals("tokens") ? new fixtures.Token[3] : new Object[3];
                String result;
                try { result="value="+(op.equals("store")?fixtures.Case.storeBoth(a,b,t):fixtures.Case.repeat(a,b,t,2)); }
                catch (Throwable e) { result=e.getClass().getName(); }
                System.out.println(op+":"+nil+":"+shape+":"+result+":"+state(a,b,t));
            }
        }
    }
}
