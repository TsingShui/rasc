public class ProjectionDriver {
    public static void main(String[] args) {
        System.setProperty("diana.detached.initialized", "no");
        try { fixtures.Projection.newDetached(-1); } catch(Throwable e) { System.out.println(e.getClass().getName() + ":" + System.getProperty("diana.detached.initialized")); }
        fixtures.Detached[][][] empty = fixtures.Projection.newDetached(2);
        System.out.println(empty.length + ":" + (empty[0] == null) + ":" + empty.getClass().getName() + ":" + System.getProperty("diana.detached.initialized"));
        System.out.println(fixtures.Projection.isDetached(empty) + ":" + fixtures.Projection.isDetached(null) + ":" + fixtures.Projection.isDetached(new Object()));
        fixtures.Projection.unusedCast(empty);
        try { fixtures.Projection.unusedCast(new Object()); } catch(Throwable e) { System.out.println(e.getClass().getName() + ":" + System.getProperty("diana.detached.initialized")); }
        System.out.println(fixtures.Projection.callRows(3) + ":" + System.getProperty("diana.detached.initialized"));
        System.out.println(Integer.toHexString(Float.floatToRawIntBits(fixtures.Projection.callFloat())));
        fixtures.Detached.touch();
        System.out.println(System.getProperty("diana.detached.initialized"));
        fixtures.Detached first = new fixtures.Detached(); fixtures.Detached second = new fixtures.Detached();
        fixtures.Detached[][][] a = new fixtures.Detached[][][]{{{first}}};
        fixtures.Detached[][][] b = new fixtures.Detached[][][]{{{second}}};
        System.out.println(fixtures.Projection.castLeaf(a) == first);
        try { fixtures.Projection.castLeaf(null); } catch(Throwable e) { System.out.println(e.getClass().getName()); }
        System.out.println(fixtures.Projection.swap(a,b,0) == first);
        System.out.println(fixtures.Projection.swap(a,b,1) == second);
        System.out.println(fixtures.Projection.swap(a,b,2) == first);
        System.out.println(fixtures.Projection.nested(new int[][]{{17,-2}},0,1));
        try { fixtures.Projection.nested(new int[][]{null},0,0); } catch(Throwable e) { System.out.println(e.getClass().getName()); }
        try { fixtures.Projection.nested(new int[][]{{1}},0,3); } catch(Throwable e) { System.out.println(e.getClass().getName()); }
        String[] strings = new String[]{"old"};
        System.out.println(fixtures.Projection.stringsGet(strings,0));
        fixtures.Projection.stringsStore(strings,"next");
        System.out.println(strings[0]);
        try { fixtures.Projection.stringsStore(strings,new Object()); } catch(Throwable e) { System.out.println(e.getClass().getName() + ":" + strings[0]); }
        try { fixtures.Projection.stringsStore(null,new Object()); } catch(Throwable e) { System.out.println(e.getClass().getName()); }
        try { fixtures.Projection.stringsStore(new String[0],new Object()); } catch(Throwable e) { System.out.println(e.getClass().getName()); }
        int[][] matrix = new int[][]{null}; int[] row = new int[]{7};
        fixtures.Projection.matrixStore(matrix,row); System.out.println(matrix[0] == row);
        try { fixtures.Projection.matrixStore(matrix,"bad"); } catch(Throwable e) { System.out.println(e.getClass().getName() + ":" + matrix[0][0]); }
        System.out.println(java.util.Arrays.toString(fixtures.Projection.filledTypedStrings("first","second")));
        System.out.println(java.util.Arrays.toString(fixtures.Projection.filledTypedStrings(null,"next")));
    }
}
