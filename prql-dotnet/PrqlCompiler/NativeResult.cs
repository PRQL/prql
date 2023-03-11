using System;

namespace Prql.Compiler
{
    internal struct NativeResult
    {
        public string Output;
        public IntPtr Messages;
        public int MessagesLen;
    }
}