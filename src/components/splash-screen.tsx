import { useEffect, useState } from 'react'

interface SplashScreenProps {
  onLoaded: () => void
}

export function SplashScreen({ onLoaded }: SplashScreenProps) {
  const [phase, setPhase] = useState<'loading' | 'ready'>('loading')

  useEffect(() => {
    const timer = setTimeout(() => {
      setPhase('ready')
    }, 800)

    return () => clearTimeout(timer)
  }, [])

  useEffect(() => {
    if (phase === 'ready') {
      const timer = setTimeout(() => onLoaded(), 300)
      return () => clearTimeout(timer)
    }
  }, [phase, onLoaded])

  return (
    <div
      className={`fixed inset-0 z-50 flex flex-col items-center justify-center bg-background transition-opacity duration-300 ${
        phase === 'ready' ? 'opacity-0' : 'opacity-100'
      }`}
    >
      <div className="flex flex-col items-center gap-6">
        <div className="relative">
          <div className="w-16 h-16 rounded-2xl bg-primary flex items-center justify-center shadow-lg">
            <span className="text-2xl font-bold text-primary-foreground">S</span>
          </div>
          <div className="absolute -bottom-1 -right-1 w-5 h-5 bg-green-500 rounded-full border-2 border-background" />
        </div>
        <div className="text-center space-y-1">
          <h1 className="text-xl font-semibold text-foreground">SquireCLI</h1>
          <p className="text-sm text-muted-foreground">AI-powered code assistant</p>
        </div>
        <div className="w-48 h-1.5 bg-muted rounded-full overflow-hidden">
          <div className="h-full bg-primary rounded-full animate-pulse" style={{ width: '60%' }} />
        </div>
      </div>
    </div>
  )
}
