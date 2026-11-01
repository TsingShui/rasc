public class Driver {
    static class Alternate extends fixtures.Reader {
        public int value(int index) { return super.value(index) + 20; }
    }
    static void observe(String op, boolean nullBox, String shape, int index, boolean failing) {
        fixtures.Box b = nullBox ? null : new fixtures.Box(); if (b != null) b.n = 3;
        int[] a = shape.equals("null") ? null : shape.equals("empty") ? new int[0] : new int[2];
        fixtures.Dep.target = b; fixtures.Dep.calls = 0; fixtures.Dep.events = "";
        fixtures.Dep.fail = failing ? (op.equals("repeat") ? 1 : index) : -99;
        String result;
        try {
            if (op.equals("combine")) result = "value=" + fixtures.Case.combine(b,a,index);
            else if (op.equals("repeat")) result = "value=" + fixtures.Case.repeat(b,a,index);
            else if (op.startsWith("invoke")) {
                fixtures.Reader receiver = op.equals("invoke-null") ? null : op.equals("invoke-child") ? new Alternate() : new fixtures.Reader();
                result = "value=" + fixtures.Case.invoke(receiver,b,a,index);
            }
            else { fixtures.Case.unused(b,a); result = "normal"; }
        } catch (Throwable e) { result = e.getClass().getName(); }
        System.out.println(op+":"+nullBox+":"+shape+":"+index+":"+failing+":"+result+":n="+(b==null?"null":b.n)+":a="+java.util.Arrays.toString(a)+":calls="+fixtures.Dep.calls+":events="+fixtures.Dep.events);
    }
    public static void main(String[] args) {
        for (boolean nil : new boolean[]{false,true}) for (String shape : new String[]{"values","empty","null"}) {
            for (int i : new int[]{0,1,-1}) for (boolean fail : new boolean[]{false,true}) observe("combine",nil,shape,i,fail);
            for (int count : new int[]{0,1,3}) for (boolean fail : new boolean[]{false,true}) observe("repeat",nil,shape,count,fail);
            observe("unused",nil,shape,0,false);
        }
        observe("invoke-base",false,"values",0,false);
        observe("invoke-child",false,"values",0,false);
        observe("invoke-null",false,"values",0,false);
        observe("invoke-base",true,"values",0,false);
        observe("invoke-base",false,"null",0,false);
        observe("invoke-base",false,"empty",0,false);
        observe("invoke-base",false,"values",0,true);
        observe("invoke-base",false,"values",1,false);
    }
}
