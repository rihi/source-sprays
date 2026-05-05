export default function myInitializer () {
	return {
		onStart: () => {
			// called when the loading starts
		},
		onProgress: ({current, total}) => {
			// the progress while loading, will be called periodically.
			// "current" will contain the number of bytes of the WASM already loaded
			// "total" will either contain the total number of bytes expected for the WASM, or if the server did not provide
			//   the content-length header it will contain 0.
		},
		onComplete: () => {
			// called when the initialization is complete (successfully or failed)
		},
		onSuccess: (wasm) => {
			// called when the initialization is completed successfully, receives the `wasm` instance
		},
		onFailure: (error) => {
			// called when the initialization is completed with an error, receives the `error`
			window.failApp(error)
		}
	}
};