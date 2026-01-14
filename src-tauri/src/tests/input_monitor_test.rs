// Pomolice - Input Monitor Tests
// Phase 4: Test input monitoring module

#[cfg(test)]
mod input_monitor_tests {
    use crate::input_monitor::{InputDetectedPayload, InputEventType};

    #[test]
    fn test_input_event_type_keyboard_serialization() {
        let event = InputEventType::Keyboard;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("Keyboard"));

        // Test that it can be deserialized back
        let deserialized: InputEventType = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            InputEventType::Keyboard => (),
            _ => panic!("Expected Keyboard variant"),
        }
    }

    #[test]
    fn test_input_event_type_mouse_click_serialization() {
        let event = InputEventType::MouseClick;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("MouseClick"));

        let deserialized: InputEventType = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            InputEventType::MouseClick => (),
            _ => panic!("Expected MouseClick variant"),
        }
    }

    #[test]
    fn test_input_event_type_mouse_move_serialization() {
        let event = InputEventType::MouseMove;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("MouseMove"));

        let deserialized: InputEventType = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            InputEventType::MouseMove => (),
            _ => panic!("Expected MouseMove variant"),
        }
    }

    #[test]
    fn test_input_detected_payload_serialization() {
        let payload = InputDetectedPayload {
            event_type: InputEventType::Keyboard,
            timestamp: 1234567890,
        };

        let serialized = serde_json::to_string(&payload).unwrap();
        assert!(serialized.contains("Keyboard"));
        assert!(serialized.contains("1234567890"));

        // Test deserialization
        let deserialized: InputDetectedPayload = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.timestamp, 1234567890);
        match deserialized.event_type {
            InputEventType::Keyboard => (),
            _ => panic!("Expected Keyboard event type"),
        }
    }

    #[test]
    fn test_input_detected_payload_with_mouse_click() {
        let payload = InputDetectedPayload {
            event_type: InputEventType::MouseClick,
            timestamp: 9876543210,
        };

        let serialized = serde_json::to_string(&payload).unwrap();
        let deserialized: InputDetectedPayload = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.timestamp, 9876543210);
        match deserialized.event_type {
            InputEventType::MouseClick => (),
            _ => panic!("Expected MouseClick event type"),
        }
    }

    #[test]
    fn test_input_detected_payload_with_mouse_move() {
        let payload = InputDetectedPayload {
            event_type: InputEventType::MouseMove,
            timestamp: 5555555555,
        };

        let serialized = serde_json::to_string(&payload).unwrap();
        let deserialized: InputDetectedPayload = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.timestamp, 5555555555);
        match deserialized.event_type {
            InputEventType::MouseMove => (),
            _ => panic!("Expected MouseMove event type"),
        }
    }

    #[test]
    fn test_payload_timestamp_is_u64() {
        let payload = InputDetectedPayload {
            event_type: InputEventType::Keyboard,
            timestamp: u64::MAX,
        };

        // Should not panic with max u64 value
        let serialized = serde_json::to_string(&payload).unwrap();
        let deserialized: InputDetectedPayload = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.timestamp, u64::MAX);
    }

    #[test]
    fn test_payload_clone() {
        let original = InputDetectedPayload {
            event_type: InputEventType::Keyboard,
            timestamp: 1234567890,
        };

        let cloned = original.clone();
        assert_eq!(cloned.timestamp, original.timestamp);
    }

    #[test]
    fn test_event_type_clone() {
        let original = InputEventType::MouseClick;
        let cloned = original.clone();

        // Both should serialize to the same value
        let original_ser = serde_json::to_string(&original).unwrap();
        let cloned_ser = serde_json::to_string(&cloned).unwrap();
        assert_eq!(original_ser, cloned_ser);
    }

    // Note: Testing actual input monitoring (start_monitoring/stop_monitoring)
    // requires real input devices and is difficult to test in CI/CD environments.
    // These functions are tested manually during development and integration testing.
}
