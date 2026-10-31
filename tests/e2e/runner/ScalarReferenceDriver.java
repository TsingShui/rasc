import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

/** Real JVM observations; there are no canned results or engine inputs. */
public final class ScalarReferenceDriver {
    public static void main(String[] args) throws Exception {
        StringBuilder out=new StringBuilder("[");
        boolean comma=false;
        for(Method m:ScalarReference.class.getDeclaredMethods()) {
            if(!Modifier.isStatic(m.getModifiers())) continue;
            Class<?>[] types=m.getParameterTypes();
            for(int sample=0;sample<3;sample++) {
                Object[] values=new Object[types.length];
                StringBuilder proto=new StringBuilder("(");
                for(int n=0;n<types.length;n++) {
                    boolean wide=types[n]==long.class;
                    proto.append(wide?'J':'I');
                    long value=n==0?(sample==0?(wide?Long.MIN_VALUE:Integer.MIN_VALUE):sample==1?(wide?Long.MAX_VALUE:Integer.MAX_VALUE):-41):sample==0?-1:sample==1?64:7;
                    // Avoid conditional numeric promotion: reflection must receive
                    // the exact Integer or Long wrapper matching each formal.
                    if(wide) values[n]=Long.valueOf(value);
                    else values[n]=Integer.valueOf((int)value);
                }
                proto.append(')').append(m.getReturnType()==long.class?'J':'I');
                Object value=m.invoke(null,values);
                if(comma)out.append(',');comma=true;
                out.append("{\"method\":\"").append(m.getName()).append(proto).append("\",\"args\":[");
                for(int n=0;n<values.length;n++) {
                    if(n>0)out.append(',');out.append('"').append(values[n]).append('"');
                }
                out.append("],\"value\":\"").append(value).append("\"}");
            }
        }
        out.append(']');System.out.println(out);
    }
}
