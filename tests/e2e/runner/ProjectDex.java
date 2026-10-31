// Tests-only independent projection through locked dexlib2; no Diana dependency.
import java.io.*;
import java.nio.file.*;
import java.util.*;
import org.jf.dexlib2.Opcodes;
import org.jf.dexlib2.dexbacked.*;
import org.jf.dexlib2.iface.*;
import org.jf.dexlib2.iface.debug.*;
import org.jf.dexlib2.iface.instruction.*;
import org.jf.dexlib2.iface.instruction.formats.ArrayPayload;
import org.jf.dexlib2.iface.instruction.formats.UnknownInstruction;
import org.jf.dexlib2.iface.reference.*;
import org.jf.dexlib2.iface.value.*;

public final class ProjectDex {
    static Map<String,Object> map(Object... args) {
        Map<String,Object> result = new LinkedHashMap<>();
        for (int i=0;i<args.length;i+=2) result.put((String)args[i], args[i+1]);
        return result;
    }
    static List<Integer> units(String value) {
        List<Integer> result = new ArrayList<>();
        for (int i=0;i<value.length();i++) result.add((int)value.charAt(i));
        return result;
    }
    static String proto(MethodReference method) {
        StringBuilder out = new StringBuilder("(");
        for (CharSequence type:method.getParameterTypes()) out.append(type);
        return out.append(')').append(method.getReturnType()).toString();
    }
    static Object reference(Reference ref) {
        if (ref instanceof StringReference) return map("kind","string","units",units(((StringReference)ref).getString()));
        if (ref instanceof TypeReference) return map("kind","type","type",((TypeReference)ref).getType());
        if (ref instanceof MethodReference) {
            MethodReference m=(MethodReference)ref;
            return map("kind","method","owner",m.getDefiningClass(),"name",m.getName(),"proto",proto(m));
        }
        if (ref instanceof FieldReference) {
            FieldReference f=(FieldReference)ref;
            return map("kind","field","owner",f.getDefiningClass(),"name",f.getName(),"type",f.getType());
        }
        if (ref instanceof MethodProtoReference) {
            MethodProtoReference p=(MethodProtoReference)ref;
            return map("kind","proto","parameters",p.getParameterTypes(),"return",p.getReturnType());
        }
        if (ref instanceof MethodHandleReference) {
            MethodHandleReference h=(MethodHandleReference)ref;
            return map("kind","method_handle","handle_type",h.getMethodHandleType(),"member",reference(h.getMemberReference()));
        }
        if (ref instanceof CallSiteReference) {
            CallSiteReference c=(CallSiteReference)ref;
            List<Object> extra=new ArrayList<>();
            for (EncodedValue v:c.getExtraArguments()) extra.add(encoded(v));
            return map("kind","call_site","name",c.getMethodName(),"handle",reference(c.getMethodHandle()),"proto",reference(c.getMethodProto()),"extra",extra);
        }
        throw new IllegalArgumentException("unknown reference " + ref.getClass());
    }
    static Object encoded(EncodedValue v) {
        if (v==null) return null;
        Object value;
        if (v instanceof ByteEncodedValue) value=((ByteEncodedValue)v).getValue();
        else if (v instanceof ShortEncodedValue) value=((ShortEncodedValue)v).getValue();
        else if (v instanceof CharEncodedValue) value=(int)((CharEncodedValue)v).getValue();
        else if (v instanceof IntEncodedValue) value=((IntEncodedValue)v).getValue();
        else if (v instanceof LongEncodedValue) value=((LongEncodedValue)v).getValue();
        else if (v instanceof FloatEncodedValue) value=String.format("%08x",Float.floatToRawIntBits(((FloatEncodedValue)v).getValue()));
        else if (v instanceof DoubleEncodedValue) value=String.format("%016x",Double.doubleToRawLongBits(((DoubleEncodedValue)v).getValue()));
        else if (v instanceof BooleanEncodedValue) value=((BooleanEncodedValue)v).getValue();
        else if (v instanceof NullEncodedValue) value=null;
        else if (v instanceof StringEncodedValue) value=units(((StringEncodedValue)v).getValue());
        else if (v instanceof TypeEncodedValue) value=((TypeEncodedValue)v).getValue();
        else if (v instanceof FieldEncodedValue) value=reference(((FieldEncodedValue)v).getValue());
        else if (v instanceof EnumEncodedValue) value=reference(((EnumEncodedValue)v).getValue());
        else if (v instanceof MethodEncodedValue) value=reference(((MethodEncodedValue)v).getValue());
        else if (v instanceof MethodTypeEncodedValue) value=reference(((MethodTypeEncodedValue)v).getValue());
        else if (v instanceof MethodHandleEncodedValue) value=reference(((MethodHandleEncodedValue)v).getValue());
        else if (v instanceof ArrayEncodedValue) {
            List<Object> values=new ArrayList<>();
            for (EncodedValue e:((ArrayEncodedValue)v).getValue()) values.add(encoded(e));
            value=values;
        } else if (v instanceof AnnotationEncodedValue) {
            AnnotationEncodedValue a=(AnnotationEncodedValue)v;
            value=map("type",a.getType(),"elements",elements(a.getElements()));
        } else throw new IllegalArgumentException("unknown encoded value " + v.getValueType());
        return map("tag",v.getValueType(),"value",value);
    }
    static Object elements(Set<? extends AnnotationElement> elements) {
        Map<String,Object> result=new TreeMap<>();
        for (AnnotationElement e:elements) result.put(e.getName(),encoded(e.getValue()));
        return result;
    }
    static Object annotations(Set<? extends Annotation> annotations) {
        List<Object> result=new ArrayList<>();
        for (Annotation a:annotations) result.add(map("visibility",a.getVisibility(),"type",a.getType(),"elements",elements(a.getElements())));
        return result;
    }
    static Object code(MethodImplementation body) {
        if (body==null) return null;
        List<Object> instructions=new ArrayList<>();
        int pc=0;
        for (Instruction i:body.getInstructions()) {
            if (i instanceof UnknownInstruction) throw new IllegalArgumentException("unknown opcode at " + pc);
            Map<String,Object> out=map("pc",pc,"opcode",i.getOpcode().name,"width",i.getCodeUnits());
            List<Integer> registers=new ArrayList<>();
            if (i instanceof RegisterRangeInstruction) {
                RegisterRangeInstruction r=(RegisterRangeInstruction)i;
                for (int n=0;n<r.getRegisterCount();n++) registers.add(r.getStartRegister()+n);
            } else if (i instanceof FiveRegisterInstruction) {
                FiveRegisterInstruction r=(FiveRegisterInstruction)i;
                int[] all={r.getRegisterC(),r.getRegisterD(),r.getRegisterE(),r.getRegisterF(),r.getRegisterG()};
                for(int n=0;n<r.getRegisterCount();n++) registers.add(all[n]);
            } else if (i instanceof OneRegisterInstruction) {
                registers.add(((OneRegisterInstruction)i).getRegisterA());
                if(i instanceof TwoRegisterInstruction) registers.add(((TwoRegisterInstruction)i).getRegisterB());
                if(i instanceof ThreeRegisterInstruction) registers.add(((ThreeRegisterInstruction)i).getRegisterC());
            }
            out.put("registers",registers);
            if(i instanceof WideLiteralInstruction) out.put("literal",((WideLiteralInstruction)i).getWideLiteral());
            if(i instanceof OffsetInstruction) out.put("target",pc+((OffsetInstruction)i).getCodeOffset());
            if(i instanceof ReferenceInstruction) out.put("reference",reference(((ReferenceInstruction)i).getReference()));
            if(i instanceof DualReferenceInstruction) out.put("reference2",reference(((DualReferenceInstruction)i).getReference2()));
            if(i instanceof ArrayPayload) {
                ArrayPayload a=(ArrayPayload)i;
                out.put("element_width",a.getElementWidth());
                out.put("elements",a.getArrayElements());
            }
            if(i instanceof SwitchPayload) {
                List<Object> cases=new ArrayList<>();
                for(SwitchElement e:((SwitchPayload)i).getSwitchElements()) cases.add(map("key",e.getKey(),"offset",e.getOffset()));
                out.put("cases",cases);
            }
            instructions.add(out); pc+=i.getCodeUnits();
        }
        List<Object> tries=new ArrayList<>();
        for(TryBlock<? extends ExceptionHandler> t:body.getTryBlocks()) {
            List<Object> handlers=new ArrayList<>();
            for(ExceptionHandler h:t.getExceptionHandlers()) handlers.add(map("type",h.getExceptionType(),"target",h.getHandlerCodeAddress()));
            tries.add(map("start",t.getStartCodeAddress(),"count",t.getCodeUnitCount(),"handlers",handlers));
        }
        List<Object> debug=new ArrayList<>();
        for(DebugItem d:body.getDebugItems()) {
            Map<String,Object> out=map("kind",d.getDebugItemType(),"pc",d.getCodeAddress());
            if(d instanceof LineNumber) out.put("line",((LineNumber)d).getLineNumber());
            if(d instanceof LocalInfo) {
                LocalInfo l=(LocalInfo)d;
                out.put("name",l.getName());out.put("type",l.getType());out.put("signature",l.getSignature());
            }
            if(d instanceof StartLocal) out.put("register",((StartLocal)d).getRegister());
            if(d instanceof EndLocal) out.put("register",((EndLocal)d).getRegister());
            if(d instanceof RestartLocal) out.put("register",((RestartLocal)d).getRegister());
            if(d instanceof SetSourceFile) out.put("source_file",((SetSourceFile)d).getSourceFile());
            debug.add(out);
        }
        return map("registers_size",body.getRegisterCount(),"insns_size",pc,"instructions",instructions,"tries",tries,"debug",debug);
    }
    static Object project(DexBackedDexFile dex) {
        List<Object> strings=new ArrayList<>();
        for(StringReference s:dex.getStringReferences()) strings.add(units(s.getString()));
        List<Object> methods=new ArrayList<>();
        for(MethodReference m:dex.getMethodSection()) methods.add(reference(m));
        List<Object> fields=new ArrayList<>();
        for(FieldReference f:dex.getFieldSection()) fields.add(reference(f));
        List<Object> protos=new ArrayList<>();
        for(MethodProtoReference p:dex.getProtoSection()) protos.add(reference(p));
        List<Object> handles=new ArrayList<>();
        for(MethodHandleReference h:dex.getMethodHandleSection()) handles.add(reference(h));
        List<Object> calls=new ArrayList<>();
        for(CallSiteReference c:dex.getCallSiteSection()) calls.add(reference(c));
        List<Object> classes=new ArrayList<>();
        int classIndex=0;
        for(ClassDef c:dex.getClassSection()) {
            List<Object> definedMethods=new ArrayList<>();
            for(Method m:c.getMethods()) {
                List<Object> params=new ArrayList<>();
                for(MethodParameter p:m.getParameters()) params.add(map("type",p.getType(),"name",p.getName(),"signature",p.getSignature(),"annotations",annotations(p.getAnnotations())));
                definedMethods.add(map("method_id_idx",((DexBackedMethod)m).getMethodIndex(),"name",m.getName(),"proto",proto(m),"flags",m.getAccessFlags(),"parameters",params,"annotations",annotations(m.getAnnotations()),"code",code(m.getImplementation())));
            }
            List<Object> definedFields=new ArrayList<>();
            for(Field f:c.getFields()) definedFields.add(map("name",f.getName(),"type",f.getType(),"flags",f.getAccessFlags(),"initial",encoded(f.getInitialValue()),"annotations",annotations(f.getAnnotations())));
            classes.add(map("class_def_idx",classIndex++,"descriptor",c.getType(),"flags",c.getAccessFlags(),"super",c.getSuperclass(),"interfaces",c.getInterfaces(),"source_file",c.getSourceFile(),"annotations",annotations(c.getAnnotations()),"fields",definedFields,"methods",definedMethods));
        }
        return map("producer","org.smali:dexlib2:2.5.2","strings",strings,"protos",protos,"method_refs",methods,"field_refs",fields,"method_handles",handles,"call_sites",calls,"classes",classes);
    }
    static void json(Object value, Writer out) throws IOException {
        if(value==null) {out.write("null");return;}
        if(value instanceof String || value instanceof Character) {
            out.write('"');String s=value.toString();
            for(int i=0;i<s.length();i++) {char c=s.charAt(i);
                if(c=='"' || c=='\\') {out.write('\\');out.write(c);}
                else if(c<32 || c>=127) out.write(String.format("\\u%04x",(int)c));
                else out.write(c);
            } out.write('"');return;
        }
        if(value instanceof Number || value instanceof Boolean) {out.write(value.toString());return;}
        if(value instanceof Map) {
            out.write('{');boolean comma=false;
            for(Object entryObject:((Map<?,?>)value).entrySet()) {Map.Entry<?,?> e=(Map.Entry<?,?>)entryObject;
                if(comma)out.write(',');comma=true;json(e.getKey(),out);out.write(':');json(e.getValue(),out);
            }out.write('}');return;
        }
        if(value instanceof Iterable) {
            out.write('[');boolean comma=false;
            for(Object item:(Iterable<?>)value) {if(comma)out.write(',');comma=true;json(item,out);}out.write(']');return;
        }
        throw new IllegalArgumentException("no JSON encoding for " + value.getClass());
    }
    public static void main(String[] args) throws Exception {
        byte[] bytes=Files.readAllBytes(Paths.get(args[0]));
        if(bytes.length<8 || bytes[4]=='0' && bytes[5]=='4') throw new IllegalArgumentException("041 requires the direct container projector, not a header overlay");
        DexBackedDexFile dex=new DexBackedDexFile(null,bytes);
        BufferedWriter out=new BufferedWriter(new OutputStreamWriter(System.out,"UTF-8"));
        json(project(dex),out);out.write('\n');out.flush();
    }
}
